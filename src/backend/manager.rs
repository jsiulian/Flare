use std::{cell::RefCell, collections::HashMap, path::Path};

use crate::storage::EncryptedSledConfigStore;
use futures::StreamExt;
use gdk_pixbuf::{
    glib::{clone, MainContext, Object, Priority},
    prelude::{Continue, ObjectExt},
};
use gio::subclass::prelude::ObjectSubclassIsExt;
use libsignal_service::ServiceAddress;
use rand::Fill;

use super::{Channel, Contact, Message};

use libsecret::{Schema, SchemaAttributeType, SchemaFlags};

use crate::ApplicationError;
use chacha20poly1305::ChaCha20Poly1305;
use encrypted_sled::{CountingNonce, EncryptionCipher};

gtk::glib::wrapper! {
    pub struct Manager(ObjectSubclass<imp::Manager>);
}

type ConfigStoreType =
    EncryptedSledConfigStore<EncryptionCipher<ChaCha20Poly1305, CountingNonce<ChaCha20Poly1305>>>;

async fn encryption_password() -> Result<Vec<u8>, ApplicationError> {
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

impl Manager {
    pub fn new() -> Manager {
        Object::new(&[]).expect("Failed to create `Manager` object.")
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn init<P: AsRef<Path>>(&self, p: &P) -> Result<(), ApplicationError> {
        use futures::{channel::oneshot, future};
        let config_store = config_store(p).await?;
        log::trace!("Setting up the manager");
        let internal = if let Ok(manager) = presage::Manager::load_registered(config_store.clone())
        {
            log::debug!("The configuration store is already valid, loading a registered account");
            manager
        } else {
            log::debug!("The config store is not valid yet, linking with a secondary device");
            let (send, receive) = MainContext::channel(Priority::default());
            receive.attach(
                None,
                clone!(@strong self as s => move |url| {
                    s.emit_by_name::<()>("link-qr-code", &[&url]);
                    Continue(false)
                }),
            );

            let (provisioning_link_tx, provisioning_link_rx) = oneshot::channel();
            let (manager, _) = future::join(
                presage::Manager::link_secondary_device(
                    config_store,
                    presage::prelude::SignalServers::Production,
                    "flare".to_string(),
                    provisioning_link_tx,
                ),
                async move {
                    match provisioning_link_rx.await {
                        Ok(url) => {
                            log::trace!("Manager wants to show QR code, emitting signal");
                            let _ = send.send(String::from(url));
                        }
                        Err(e) => log::error!("Error linking device: {e}"),
                    }
                },
            )
            .await;
            self.emit_by_name::<()>("link-finish", &[]);
            manager?
        };

        self.imp().internal.swap(&RefCell::new(Some(internal)));

        self.sync_contacts().await?;
        Ok(())
    }

    async fn sync_contacts(&self) -> Result<(), presage::Error> {
        log::trace!("Requesting contact sync");
        let _ = self.internal().request_contacts_sync().await?;
        self.imp()
            .profile
            .borrow_mut()
            .replace(self.internal().retrieve_profile().await?);
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

    pub(super) fn internal(&self) -> presage::Manager<ConfigStoreType, presage::Registered> {
        self.imp().internal()
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn setup_receive_message_loop(&self) -> Result<(), ApplicationError> {
        log::debug!("Start receiving messages");
        let messages = self.internal().receive_messages().await?;
        futures::pin_mut!(messages);
        while let Some(msg) = messages.next().await {
            let message = Message::from_content(msg, self).await;
            if let Some(channel) = message.channel() {
                let mut channels = self.imp().channels.borrow_mut();
                crate::debug!("Got from channel: {}", channel.property::<String>("title"));
                self.emit_by_name::<()>("message", &[&message]);
                if let Some(stored_channel) = channels.get(&channel.internal_hash()) {
                    log::debug!("Message from a already existing channel");
                    stored_channel.new_message(message);
                } else {
                    log::debug!("Got a message from a new channel");
                    self.emit_by_name::<()>("channel", &[&channel]);
                    channel.new_message(message);
                    channels.insert(channel.internal_hash(), channel);
                }
            } else {
                log::trace!("Message is not associated with channel");
            }
            log::debug!("Emitting message");
        }
        Ok(())
    }

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
                uuid: Some(self.internal().uuid()),
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
    pub async fn init_channels(&self) {
        let mut channels = self.imp().channels.borrow_mut();

        for contact in self.list_contacts() {
            let channel = Channel::from_contact_or_group(contact, &None, self).await;
            self.emit_by_name::<()>("channel", &[&channel]);
            channels.insert(channel.internal_hash(), channel);
        }
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::{
        glib::{once_cell::sync::Lazy, subclass::Signal},
        prelude::StaticType,
    };
    use gtk::glib;
    use presage::libsignal_service::Profile;
    use std::{cell::RefCell, collections::HashMap};

    use crate::backend::{Channel, Message};

    #[derive(Default)]
    pub struct Manager {
        pub(super) internal:
            RefCell<Option<presage::Manager<super::ConfigStoreType, presage::Registered>>>,
        #[cfg(feature = "screenshot")]
        pub(in super::super) channels: RefCell<HashMap<u64, Channel>>,
        #[cfg(not(feature = "screenshot"))]
        pub(super) channels: RefCell<HashMap<u64, Channel>>,
        pub(super) profile: RefCell<Option<Profile>>,
    }

    impl Manager {
        pub(super) fn internal(
            &self,
        ) -> presage::Manager<super::ConfigStoreType, presage::Registered> {
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
