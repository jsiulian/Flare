use std::{cell::RefCell, collections::HashMap, path::Path, time::Duration};

use chacha20poly1305::ChaCha20Poly1305;
use encrypted_sled::{CountingNonce, EncryptionCipher};
use gdk::prelude::*;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{clone, MainContext, Object, Priority};
use libsecret::{
    prelude::ServiceExtManual, traits::CollectionExt, Collection, CollectionFlags, Schema,
    SchemaAttributeType, SchemaFlags, Service, ServiceFlags, COLLECTION_DEFAULT,
};
use libsignal_service::{
    content::{ContentBody, Metadata},
    groups_v2::Group,
    prelude::{Content, GroupMasterKey, Uuid},
    proto::{AttachmentPointer, DataMessage, GroupContextV2},
    sender::{AttachmentSpec, AttachmentUploadError},
    ServiceAddress,
};
use presage::{MessageStore, Thread};
use rand::Fill;

use super::{manager_thread::ManagerThread, Channel, Contact, Message};
use crate::storage::EncryptedSledConfigStore;
use crate::ApplicationError;

const MESSAGE_BOUND: usize = 10;
const INIT_CHANNELS_SLEEP_SECS: u64 = 10;

gtk::glib::wrapper! {
    pub struct Manager(ObjectSubclass<imp::Manager>);
}

type ConfigStoreType =
    EncryptedSledConfigStore<EncryptionCipher<ChaCha20Poly1305, CountingNonce<ChaCha20Poly1305>>>;

// Similar to https://gitlab.gnome.org/GNOME/geary/-/blob/main/src/client/application/secret-mediator.vala#L112
async fn ensure_secret_unlocked() -> Result<(), ApplicationError> {
    log::trace!("Ensuring the default collection is unlocked");
    let service = Service::get_future(ServiceFlags::OPEN_SESSION).await?;
    let collection =
        Collection::for_alias_future(Some(&service), &COLLECTION_DEFAULT, CollectionFlags::NONE)
            .await?;
    if collection.is_some() && collection.as_ref().unwrap().is_locked() {
        log::trace!("Unlocking the default collection");
        service
            .unlock_future(&[collection.unwrap()
                .dynamic_cast()
                .expect("Failed to cast `Collection` to `DBusProxy`")])
            .await?;
    }
    Ok(())
}

async fn encryption_password() -> Result<Vec<u8>, ApplicationError> {
    ensure_secret_unlocked().await?;

    let schema = Schema::new(
        crate::config::APP_ID,
        SchemaFlags::NONE,
        HashMap::from([("encryption", SchemaAttributeType::String)]),
    );
    log::trace!("Looking up password from libsecret");
    // Lookup with future is broken, see https://gitlab.gnome.org/GNOME/libsecret/-/issues/58
    let stored =
        libsecret::password_lookup_sync(Some(&schema), HashMap::new(), gio::Cancellable::NONE)?;
    if let Some(store) = stored {
        log::trace!("Password already stored in libsecret");
        Ok(hex::decode(String::from(store)).expect("Stored password to be hex"))
    } else {
        log::trace!("Generating password and storing it");
        let key_bytes: &mut [u8; 32] = &mut [0; 32];
        key_bytes
            .try_fill(&mut rand::thread_rng())
            .expect("Failed to generate random values");
        let key_str = hex::encode(&key_bytes);
        libsecret::password_store_future(
            Some(&schema),
            HashMap::new(),
            None,
            "Encryption password",
            &key_str,
        )
        .await?;
        Ok(key_bytes.to_vec())
    }
}

async fn config_store<P: AsRef<Path>>(p: &P) -> Result<ConfigStoreType, ApplicationError> {
    let path = p.as_ref();
    log::trace!("Initialize config store at {}", path.to_string_lossy());

    if path.exists() && !path.is_dir() {
        log::error!(
            "Store location already exists and is not a directory: {}",
            path.to_string_lossy()
        );
        return Err(ApplicationError::ConfigurationError(
            crate::ConfigurationError::DbPathNoFolder(path.to_owned()),
        ));
    }

    let cipher = {
        use chacha20poly1305::{Key, Nonce};
        let mut key = Key::default();
        key.copy_from_slice(&encryption_password().await?);
        encrypted_sled::EncryptionCipher::<ChaCha20Poly1305, _>::new(
            key,
            encrypted_sled::CountingNonce::new(Nonce::default()),
            encrypted_sled::EncryptionMode::default(),
        )
    };
    Ok(EncryptedSledConfigStore::new(path, cipher)?)
}

impl std::default::Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Manager {
        Object::new(&[]).expect("Failed to create `Manager` object.")
    }

    pub fn clear(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing the manager");
        if let Some(config_store) = self.imp().config_store.borrow().as_ref() {
            config_store.clear()?;
        }
        Ok(())
    }

    pub fn clear_messages(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing messages from the manager");
        if let Some(config_store) = self.imp().config_store.borrow().as_ref() {
            config_store.clear_messages()?;
        }
        Ok(())
    }

    pub fn save_message(
        &self,
        message: Content,
        recipient: Option<impl Into<ServiceAddress>>,
    ) -> Result<(), ApplicationError> {
        log::trace!("Saving a message");
        let mut borrow_mut = self.imp().config_store.borrow_mut();
        if let Some(config_store) = borrow_mut.as_mut() {
            config_store.save_message(message, recipient)?;
        }
        Ok(())
    }

    pub async fn message(
        &self,
        thread: &Thread,
        timestamp: u64,
    ) -> Result<Option<Message>, ApplicationError> {
        crate::trace!(
            "Querying message by thread: {:?}, timestamp: {}",
            thread,
            timestamp
        );
        let content = {
            if let Some(config_store) = self.imp().config_store.borrow().as_ref() {
                let content = config_store.message(thread, timestamp)?;
                if let Some(content) = content {
                    Ok::<_, ApplicationError>(Some(content))
                } else {
                    Ok(None)
                }
            } else {
                log::warn!("Query message by id without config store being set up");
                Ok(None)
            }
        }?;
        if let Some(content) = content {
            let msg = Message::from_content(content, self).await;
            log::trace!(
                "Found message: {}",
                msg.property::<String>("textual-description")
            );
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }

    pub fn messages(
        &self,
        thread: &Thread,
        from: Option<u64>,
    ) -> Result<impl Iterator<Item = Content>, ApplicationError> {
        crate::trace!("Querying message by thread: {:?}, from {:?}", thread, from);
        if let Some(config_store) = self.imp().config_store.borrow().as_ref() {
            Ok(config_store.messages(thread, from)?)
        } else {
            log::error!("Query messages by contact without config store being set up");
            // TODO: Error?
            panic!("Query messages by contact without config store being set up");
        }
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn init<P: AsRef<Path>>(&self, p: &P) -> Result<(), ApplicationError> {
        use futures::channel::oneshot;
        use futures::{select, FutureExt};
        use tokio::sync::mpsc;
        let config_store = config_store(p).await?;

        log::trace!("Setting up the config store");
        self.imp()
            .config_store
            .swap(&RefCell::new(Some(config_store.clone())));

        log::trace!("Setting up the manager");
        let (provisioning_link_tx, provisioning_link_rx) = oneshot::channel();
        let (error_tx, error_rx) = oneshot::channel();

        let (send_content, mut receive_content) = mpsc::channel(MESSAGE_BOUND);
        let (send_error, mut receive_error) = mpsc::channel(MESSAGE_BOUND);

        let (send, receive) = MainContext::channel(Priority::default());
        receive.attach(
            None,
            clone!(@strong self as s => move |url| {
                s.emit_by_name::<()>("link-qr-code", &[&url]);
                Continue(false)
            }),
        );

        let context = MainContext::default();
        context.spawn_local(async move {
            log::trace!("Awaiting for provisioning link");
            match provisioning_link_rx.await {
                Ok(url) => {
                    log::trace!("Manager wants to show QR code, emitting signal");
                    let _ = send.send(String::from(url));
                }
                Err(_e) => log::trace!("Manager is already linked"),
            }
        });

        let internal = ManagerThread::new(
            config_store,
            self.imp().settings.string("link-device-name").to_string(),
            provisioning_link_tx,
            error_tx,
            send_content,
            send_error,
        )
        .await;

        log::trace!("Awaiting for error linking");
        match error_rx.await {
            Ok(err) => {
                log::error!("Got error linking device: {}", err);
                return Err(err.into());
            }
            Err(_e) => log::trace!("Manager setup successfull"),
        }

        if internal.is_none() {
            if let Some(error_opt) = receive_error.recv().await {
                log::error!("Got error after linking device: {}", error_opt);
                return Err(error_opt.into());
            }
        }

        self.imp().internal.swap(&RefCell::new(internal));

        self.emit_by_name::<()>("link-finish", &[]);

        self.sync_contacts().await?;

        let mut channels_init = self.init_channels().await;

        crate::info!("Own uuid: {:?}", self.uuid());
        log::debug!("Start receiving messages");
        'outer: loop {
            let mut init_channels_sleep =
                gtk::glib::timeout_future(Duration::from_secs(INIT_CHANNELS_SLEEP_SECS)).fuse();
            select! {
                () = &mut init_channels_sleep => {
                    if !channels_init {
                        channels_init = self.init_channels().await;
                    }
                }
                error_opt = receive_error.recv().fuse() => {
                    if error_opt.is_none() {
                        break 'outer;
                    }
                    return Err(error_opt.unwrap().into());
                }
                msg_opt = receive_content.recv().fuse() => {
                    if msg_opt.is_none() {
                        break 'outer;
                    }
                    let msg = msg_opt.unwrap();
                    let message = Message::from_content(msg, self).await;
                    if let Some(channel) = message.channel() {
                        let channel = {
                            let channels = self.imp().channels.borrow();
                            crate::debug!("Got from channel: {}", channel.property::<String>("title"));
                            if let Some(stored_channel) = channels.get(&channel.internal_hash()) {
                                log::debug!("Message from a already existing channel");
                                stored_channel.clone()
                            } else {
                                drop(channels);
                                log::debug!("Got a message from a new channel");
                                if self.try_emit_by_name::<()>("channel", &[&channel]).is_err() {
                                    break 'outer;
                                }
                                let mut channels_mut = self.imp().channels.borrow_mut();
                                channels_mut.insert(channel.internal_hash(), channel.clone());
                                if let Some(ctx) = channel.group_context() {
                                    log::trace!("New channel is a group, inserting into store");
                                    // TODO: Error?
                                    let _ = self.insert_group(ctx);
                                }
                                channel
                            }
                        };
                        if channel.new_message(message).await.is_err() {
                            break 'outer;
                        }
                    } else {
                        log::trace!("Message is not associated with channel");
                    }
                    log::debug!("Emitting message");
                }
                complete => break,
            };
        }
        Ok(())
    }

    fn insert_group(&self, ctx: GroupContextV2) -> Result<(), ApplicationError> {
        if let Some(key) = ctx.master_key {
            self.store().save_group(&key)?
        }
        Ok(())
    }

    fn store(&self) -> ConfigStoreType {
        self.imp()
            .config_store
            .borrow()
            .as_ref()
            .expect("Config store to be set up")
            .clone()
    }

    async fn sync_contacts(&self) -> Result<(), presage::Error> {
        log::trace!("Requesting contact sync");
        self.internal().request_contacts_sync().await?;
        // let profile = self.internal().retrieve_profile().await?;
        // self.imp().profile.borrow_mut().replace(profile);
        Ok(())
    }

    pub(super) fn profile_name(&self) -> String {
        gettextrs::gettext("You")
        // Profile not yet working in backend.
        // self.imp()
        //     .profile
        //     .borrow()
        //     .as_ref()
        //     .expect("Profile to be synced")
        //     .name
        //     .as_ref()
        //     .map(|n| {
        //         format!(
        //             "{} {}",
        //             n.given_name,
        //             n.family_name.as_ref().unwrap_or(&"".to_string())
        //         )
        //     })
        //     .unwrap_or(gettextrs::gettext("No Name"))
    }

    fn internal(&self) -> ManagerThread {
        self.imp().internal()
    }

    pub(super) fn available_channels(&self) -> Vec<Channel> {
        self.imp().channels.borrow().values().cloned().collect()
    }

    #[cfg(not(feature = "screenshot"))]
    pub fn list_contacts(&self) -> Vec<Contact> {
        self.internal()
            .get_contacts()
            .map(|c| {
                c.filter(|c| !c.blocked && !c.archived)
                    .map(|c| Contact::from_contact(c, self))
                    .collect::<Vec<Contact>>()
            })
            .unwrap_or_default()
    }

    pub fn self_contact(&self) -> Contact {
        let presage_contact = presage::prelude::Contact {
            address: ServiceAddress {
                uuid: Some(self.uuid()),
                phonenumber: None,
                relay: None,
            },
            name: "".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        };
        Contact::from_contact(presage_contact, self)
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn init_channels(&self) -> bool {
        log::trace!("Trying to initialize channels");
        let mut manager = self.internal();
        let _ = manager.sync_contacts().await;
        let mut to_load = vec![];
        for contact in self.list_contacts() {
            log::trace!("Got a contact from the storage");
            let channel = Channel::from_contact_or_group(contact, &None, self).await;
            self.emit_by_name::<()>("channel", &[&channel]);
            let mut channels = self.imp().channels.borrow_mut();
            to_load.push(channel.clone());
            channels.insert(channel.internal_hash(), channel);
        }
        // TODO: Error handling?
        for key in self.store().get_groups().unwrap_or_default() {
            let group = manager.get_group_v2(GroupMasterKey::new(key)).await;
            if let Ok(group) = group {
                let channel = Channel::from_group(
                    group,
                    &GroupContextV2 {
                        master_key: Some(key.into()),
                        revision: None,
                        group_change: None,
                    },
                    self,
                )
                .await;
                self.emit_by_name::<()>("channel", &[&channel]);
                let mut channels = self.imp().channels.borrow_mut();
                to_load.push(channel.clone());
                channels.insert(channel.internal_hash(), channel);
            }
        }
        let something_loaded = !to_load.is_empty();
        for c in to_load {
            c.load_last(
                self.imp()
                    .settings
                    .int("messages-initial-load")
                    .try_into()
                    .unwrap_or(1),
            )
            .await;
        }
        something_loaded
    }
}

impl Manager {
    pub(super) async fn get_group_v2(
        &self,
        master_key: GroupMasterKey,
    ) -> Result<Group, presage::Error> {
        log::trace!("`Manager::get_group_v2`start");
        let r = self.internal().get_group_v2(master_key).await;
        log::trace!("`Manager::get_group_v2`finished");
        r
    }

    pub(super) async fn send_message(
        &self,
        recipient_addr: impl Into<ServiceAddress> + std::clone::Clone,
        message: impl Into<ContentBody>,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_message` start");
        let meta = Metadata {
            sender: ServiceAddress {
                uuid: Some(self.uuid()),
                phonenumber: None,
                relay: None,
            },
            sender_device: 0,
            timestamp,
            needs_receipt: false,
        };
        let body = message.into();
        let r = self
            .internal()
            .send_message(recipient_addr.clone(), body.clone(), timestamp)
            .await;
        let msg = Content {
            metadata: meta,
            body,
        };
        self.save_message(msg, Some(recipient_addr))?;
        log::trace!("`Manager::send_message`finished");
        Ok(r?)
    }

    pub(super) async fn send_message_to_group(
        &self,
        recipient_addr: impl IntoIterator<Item = ServiceAddress>,
        message: DataMessage,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_message_to_group` start");
        let meta = Metadata {
            sender: ServiceAddress {
                uuid: Some(self.uuid()),
                phonenumber: None,
                relay: None,
            },
            sender_device: 0,
            timestamp,
            needs_receipt: false,
        };
        let r = self
            .internal()
            .send_message_to_group(recipient_addr, message.clone(), timestamp)
            .await;
        let msg = Content {
            metadata: meta,
            body: ContentBody::DataMessage(message),
        };
        self.save_message(msg, None::<ServiceAddress>)?;
        log::trace!("`Manager::send_message_to_group` finish");
        Ok(r?)
    }

    pub(super) fn get_contact_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<presage::prelude::Contact>, presage::Error> {
        log::trace!("`Manager::get_contact_by_id` start");
        let r = self.internal().get_contact_by_id(id);
        log::trace!("`Manager::get_contact_by_id` finished");
        r
    }

    pub(super) async fn get_attachment(
        &self,
        attachment_pointer: &AttachmentPointer,
    ) -> Result<Vec<u8>, presage::Error> {
        log::trace!("`Manager::get_attachment` start");
        let r = self.internal().get_attachment(attachment_pointer).await;
        log::trace!("`Manager::get_attachment` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) fn uuid(&self) -> Uuid {
        self.internal().uuid()
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn upload_attachments(
        &self,
        attachments: Vec<(AttachmentSpec, Vec<u8>)>,
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, presage::Error> {
        log::trace!("`Manager::upload_attachment` start");
        let r = self.internal().upload_attachments(attachments).await;
        log::trace!("`Manager::upload_attachment` finished");
        r
    }
}

mod imp {
    use std::{cell::RefCell, collections::HashMap};

    use gdk::prelude::StaticType;
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gio::Settings;
    use glib::{once_cell::sync::Lazy, subclass::Signal};

    use crate::{
        backend::{manager_thread::ManagerThread, Channel, Message},
        config::APP_ID,
    };

    pub struct Manager {
        pub(super) internal: RefCell<Option<ManagerThread>>,
        pub(super) config_store: RefCell<Option<super::ConfigStoreType>>,
        #[cfg(feature = "screenshot")]
        pub(in super::super) channels: RefCell<HashMap<u64, Channel>>,
        #[cfg(not(feature = "screenshot"))]
        pub(super) channels: RefCell<HashMap<u64, Channel>>,
        // pub(super) profile: RefCell<Option<Profile>>,
        pub(super) settings: Settings,
    }

    impl Default for Manager {
        fn default() -> Self {
            Self {
                internal: Default::default(),
                config_store: Default::default(),
                channels: Default::default(),
                settings: Settings::new(APP_ID),
            }
        }
    }

    impl Manager {
        pub(super) fn internal(&self) -> ManagerThread {
            self.internal
                .borrow()
                .as_ref()
                .expect("Manager internal not yet set")
                .clone()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Manager {
        const NAME: &'static str = "FlManager";
        type Type = super::Manager;
    }

    impl ObjectImpl for Manager {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![
                    Signal::builder(
                        "message",
                        &[Message::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder(
                        "channel",
                        &[Channel::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder(
                        "link-qr-code",
                        &[String::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder("link-finish", &[], <()>::static_type().into()).build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
}
