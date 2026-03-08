use crate::prelude::*;

use std::{collections::HashMap, ops::Bound, path::Path};

use gio::Application;
use gio::Settings;
use libsignal_service::protocol::DeviceId;
use libsignal_service::{
    Profile,
    content::ContentBody,
    proto::{AttachmentPointer, DataMessage, GroupContextV2},
    protocol::ServiceId,
    sender::{AttachmentSpec, AttachmentUploadError},
    websocket::account::DeviceInfo,
};
use oo7::Keyring;
use presage::model::groups::Group;
use presage::store::{ContentsStore, StateStore, Thread};
use rand::distr::SampleString;
use url::Url;

use super::{Channel, Contact, Message, manager_thread::ManagerThread};
use crate::backend::message::{DisplayMessage, DisplayMessageExt};
use crate::{ApplicationError, gspawn, tspawn};

const MESSAGE_BOUND: usize = 100;
const MESSAGES_INITIAL_LOAD: usize = 1;
const SCHEMA_ATTRIBUTE: &str = "xdg:schema";
const ATTRIBUTE_PASSWORD: (&str, &str) = ("type", "password");
const SECRET_LENGTH: usize = 64;

gtk::glib::wrapper! {
    /// The manager is the core of the logic of Flare.
    ///
    /// It is mostly a wrapper around [ManagerThread] (which is itself a wrapper around [presage::Manager]).
    /// It also has other functions, like caching the channels which are in use or sending notifications.
    pub struct Manager(ObjectSubclass<imp::Manager>);
}

type StoreType = presage_store_sqlite::SqliteStore;
type PresageError = presage::Error<presage_store_sqlite::SqliteStoreError>;

/// Query the encryption password from the keyring, storing one if none exists.
async fn encryption_password() -> Result<String, ApplicationError> {
    let keyring = Keyring::new().await?;
    keyring.unlock().await?;
    let attributes = HashMap::from([
        (SCHEMA_ATTRIBUTE, crate::config::BASE_ID),
        ATTRIBUTE_PASSWORD,
    ]);

    log::trace!("Looking up password from libsecret");
    let items = keyring.search_items(&attributes).await?;
    let item = items.first();

    if let Some(item) = item {
        log::trace!("Password found");
        if item.is_locked().await? {
            log::trace!("Item is locked. Unlocking.");
            item.unlock().await?;
        }
        let secret_bytes = item.secret().await?;
        // Should normally not be lossy, but just in case
        let secret = String::from_utf8_lossy(&secret_bytes).into_owned();
        Ok(secret)
    } else {
        log::trace!("Password not found, creating password");
        let distribution = rand::distr::StandardUniform {};
        let secret = distribution.sample_string(&mut rand::rng(), SECRET_LENGTH);
        let secret_bytes = secret.as_bytes();
        log::trace!("Storing password");
        keyring
            .create_item(
                "Flare: Encryption password",
                &attributes,
                secret_bytes,
                true,
            )
            .await?;
        Ok(secret)
    }
}

/// Creating the configuration store at the specified path.
async fn config_store<P: AsRef<Path>>(p: &P) -> Result<StoreType, ApplicationError> {
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

    if !path.exists()
        && let Err(e) = std::fs::create_dir_all(path)
    {
        return Err(ApplicationError::ConfigurationError(
            crate::ConfigurationError::CannotCreateDbFolder(path.to_owned(), e),
        ));
    }

    let passphrase = tspawn!(async { encryption_password().await })
        .await
        .expect("Failed tokio join")?;
    let path = path.to_str().expect("Invalid sqlite store path").to_owned() + "db.sqlite";

    Ok(tspawn!(async move {
        presage_store_sqlite::SqliteStore::open_with_passphrase(
            &path,
            Some(&passphrase),
            presage::model::identity::OnNewIdentity::Trust,
        )
        .await
    })
    .await
    .expect("Failed tokio join")?)
}

impl Manager {
    pub fn new(application: Application) -> Manager {
        let s: Self = Object::new::<Self>();
        s.imp().application.borrow_mut().replace(application);
        s
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub async fn send_notification(&self, id: Option<String>, notification: &gio::Notification) {
        if self.imp().settings.boolean("notifications")
            && let Some(application) = self.application()
        {
            log::trace!("Sending a notification");
            application.send_notification(id.as_deref(), notification);
        }
    }

    pub fn application(&self) -> Option<Application> {
        self.imp().application.borrow().clone()
    }

    pub async fn clear_registration(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing the manager");
        let mut store = self.store();
        tspawn!(async move { store.clear_registration().await })
            .await
            .expect("Failed to spawn tokio")?;
        Ok(())
    }

    pub async fn clear_contents(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing the manager content");
        let mut store = self.store();
        tspawn!(async move { store.clear_contents().await })
            .await
            .expect("Failed to spawn tokio")?;
        Ok(())
    }

    pub async fn clear_messages(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing messages from the manager");
        let mut store = self.store();
        tspawn!(async move { store.clear_messages().await })
            .await
            .expect("Failed to spawn tokio")?;
        Ok(())
    }

    pub async fn clear_channel_messages(&self, channel: &Channel) -> Result<(), ApplicationError> {
        crate::trace!(
            "Clearing channel messages the manager for: {}",
            channel.title()
        );
        let thread = channel.thread();
        let mut store = self.store();
        tspawn!(async move { store.clear_thread(&thread).await })
            .await
            .expect("Failed to spawn tokio")?;
        Ok(())
    }

    pub async fn submit_recaptcha_challenge<S: AsRef<str>>(
        &self,
        token: S,
        captcha: S,
    ) -> Result<(), ApplicationError> {
        let token = token.as_ref().to_owned();
        let mut captcha = captcha.as_ref().to_owned();
        if captcha.starts_with("signalcaptcha://") {
            log::trace!("Captcha is the full link. Remove unneeded thigs.");
            // The warning is deactivated by default on newer Rust versions, and the applied fix does not even compile.
            #[allow(clippy::assigning_clones)]
            if let Some((_, c)) = captcha.split_once(".challenge.") {
                captcha = c.to_owned();
            } else {
                log::warn!("Splitting the captcha was not successfull. Assuming it is fine");
            }
        }
        crate::trace!(
            "Submitting recaptcha challenge with token {} and captcha {}",
            token,
            captcha
        );
        let internal = self.internal();
        tspawn!(async move { internal.submit_recaptcha_challenge(token, captcha).await })
            .await
            .expect("Failed to spawn tokio")?;
        Ok(())
    }

    #[cfg(not(feature = "screenshot"))]
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
            let store = self.store();
            let thread = thread.clone();
            let content = tspawn!(async move { store.message(&thread, timestamp).await })
                .await
                .expect("Failed to spawn tokio")?;
            if let Some(content) = content {
                Ok::<_, ApplicationError>(Some(content))
            } else {
                Ok(None)
            }
        }?;
        if let Some(content) = content {
            let msg = Message::from_content(content, self).await;
            log::trace!(
                "Found message queried: {}",
                msg.as_ref()
                    .and_then(|t| t.dynamic_cast_ref::<DisplayMessage>())
                    .and_then(|t| t.textual_description())
                    .unwrap_or("No Text".to_string())
            );
            Ok(msg)
        } else {
            Ok(None)
        }
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn messages(
        &self,
        thread: &Thread,
        from: Option<u64>,
    ) -> Result<impl Iterator<Item = Content>, ApplicationError> {
        crate::trace!("Querying message by thread: {:?}, from {:?}", thread, from);
        let internal = self.internal();
        let thread = thread.clone();
        Ok(tspawn!(async move {
            internal
                .messages(
                    thread,
                    (
                        Bound::Unbounded,
                        from.map(Bound::Excluded).unwrap_or(Bound::Unbounded),
                    ),
                )
                .await
        })
        .await
        .expect("Failed to spawn tokio")?
        // .rev()
        .filter_map(|o| o.ok()))
    }

    /// Asynchronously initialize the manager. This will never return unless there is an error.
    ///
    /// This includes:
    /// - Setting up the configuration store.
    /// - Constructing the manager thread, reacting to any setup results or errors that happen.
    /// - Loading the stored channels.
    /// - Listening for messages or errors and propagating them to the correct channels.
    #[cfg(not(feature = "screenshot"))]
    pub async fn init<P: AsRef<Path>>(&self, p: &P) -> Result<(), ApplicationError> {
        use futures::channel::{mpsc, oneshot};
        use futures::{FutureExt, StreamExt, select};
        use gdk::glib::BoxedAnyObject;

        let config_store = config_store(p).await?;

        log::trace!("Setting up the config store");
        self.imp()
            .config_store
            .swap(&RefCell::new(Some(config_store.clone())));

        log::trace!("Setting up the manager");
        let (setup_results_tx, mut setup_results_rx) =
            futures::channel::mpsc::channel(MESSAGE_BOUND);
        let (error_tx, error_rx) = oneshot::channel();

        let (send_content, mut receive_content) = mpsc::unbounded();
        let (send_error, mut receive_error) = mpsc::channel(MESSAGE_BOUND);

        gspawn!(clone!(
            #[weak(rename_to = s)]
            self,
            async move {
                log::trace!("Awaiting for setup results");
                while let Some(result) = setup_results_rx.next().await {
                    s.emit_by_name::<()>("setup-result", &[&BoxedAnyObject::new(result)]);
                }
            }
        ));

        let internal = ManagerThread::new(
            config_store,
            setup_results_tx,
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
            Err(_e) => log::trace!("Manager setup successful"),
        }

        if internal.is_none()
            && let Some(error_opt) = receive_error.next().await
        {
            log::error!("Got error after linking device: {}", error_opt);
            return Err(error_opt);
        }

        self.imp().internal.swap(&RefCell::new(internal));

        // Check again if is primary, after setup is successful.
        self.notify("is-primary");

        crate::info!("Own uuid: {:?}", self.uuid());
        log::debug!("Start receiving messages");
        'outer: loop {
            use presage::model::messages::Received;
            select! {
                // Receive errors.
                error_opt = receive_error.next().fuse() => {
                    if error_opt.is_none() {
                        break 'outer;
                    }
                    self.emit_by_name::<()>("error", &[&BoxedAnyObject::new(error_opt.unwrap())]);
                }
                // Receive messages.
                msg_opt = receive_content.next().fuse() => {
                    match msg_opt {
                        None => { break 'outer },
                        Some(Received::QueueEmpty) => {
                            if !self.imp().finished_setup.get() {
                                self.imp().finished_setup.set(true);
                                self.init_channels().await;
                                self.notify("finished-setup");
                            }
                        },
                        Some(Received::Contacts) => {
                            log::trace!("Received contacts");
                            self.init_channels().await;
                        }
                        Some(Received::Content(msg)) => {
                            let message = Message::from_content(*msg, self).await;
                            if message.is_none() {
                                log::trace!("Manager ignoring empty message");
                                continue;
                            }
                            let message = message.unwrap();

                            self.imp().last_message_datetime.replace(message.property("datetime"));
                            self.notify("last-message-datetime");

                            let channel = message.channel();
                            let channel = {
                                let channels = self.imp().channels.borrow();
                                crate::debug!("Got from channel: {}", channel.property::<String>("title"));
                                if let Some(stored_channel) = channels.get(&channel.thread()) {
                                    log::debug!("Message from a already existing channel");
                                    stored_channel.clone()
                                } else {
                                    drop(channels);
                                    log::debug!("Got a message from a new channel");
                                    self.emit_by_name::<()>("channel", &[&channel]);
                                    let mut channels_mut = self.imp().channels.borrow_mut();
                                    channels_mut.insert(channel.thread(), channel.clone());
                                    channel
                                }
                            };
                            if channel.new_message(message).await.is_err() {
                                break 'outer;
                            }
                            log::debug!("Emitting message");
                        }
                    }
                }
                complete => break,
            };
        }
        Ok(())
    }

    fn store(&self) -> StoreType {
        self.imp()
            .config_store
            .borrow()
            .as_ref()
            .expect(" store to be set up")
            .clone()
    }

    #[cfg(not(feature = "screenshot"))]
    pub fn profile_name(&self) -> String {
        self.internal()
            .retrieve_profile()
            .and_then(|p| p.name)
            .as_ref()
            .map(crate::utils::format_profile_name)
            .unwrap_or_else(|| gettextrs::gettext("You"))
    }

    fn internal(&self) -> ManagerThread {
        self.imp().internal()
    }

    pub fn available_channels(&self) -> Vec<Channel> {
        self.imp().channels.borrow().values().cloned().collect()
    }

    pub(super) async fn channel_from_uuid_or_group(
        &self,
        uuid: ServiceId,
        group: &Option<GroupContextV2>,
    ) -> Channel {
        let thread = if let Some(group) = group {
            Thread::Group(
                group
                    .master_key()
                    .try_into()
                    .expect("Group master key to have the correct length"),
            )
        } else {
            Thread::Contact(uuid)
        };

        let contact = Contact::from_service_address(&uuid, self).await;
        let channel = Channel::from_contact_or_group(contact, group, self).await;
        channel.initialize_avatar().await;

        let mut known_channels = self.imp().channels.borrow_mut();
        known_channels.entry(thread).or_insert_with(|| {
            log::trace!("Got a contact from the storage");
            self.emit_by_name::<()>("channel", &[&channel]);
            channel.clone()
        });

        // No need to initialize avatar or last messages in here, will be done when initializing contacts.

        channel
    }

    pub fn channel_from_thread(&self, thread: Thread) -> Option<Channel> {
        let known_channels = self.imp().channels.borrow_mut();
        known_channels.get(&thread).cloned()
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn list_contacts(&self) -> Vec<Contact> {
        let store = self.store();
        tspawn!(async move { store.contacts().await })
            .await
            .expect("Failed to spawn tokio")
            .map(|i| {
                i.filter_map(|c| c.ok().map(|c| Contact::from_contact(c, self)))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn self_contact(&self) -> Contact {
        let presage_contact = presage::model::contacts::Contact {
            uuid: self.uuid(),
            // TODO: Get own phone number?
            phone_number: None,
            name: "".to_string(),
            verified: Default::default(),
            profile_key: vec![],
            expire_timer: 0,
            expire_timer_version: 0,
            inbox_position: 0,
            avatar: None,
        };
        Contact::from_contact(presage_contact, self)
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn init_channels(&self) -> bool {
        log::trace!("Trying to initialize channels");
        let mut to_load = vec![];

        // Construct all channels in parallel.
        let channels_futures = self.list_contacts().await.into_iter().map(|c| async move {
            let channel = Channel::from_contact_or_group(c.clone(), &None, self).await;
            c.set_channel(Some(&channel));
            channel
        });
        let loaded_channels = futures::future::join_all(channels_futures).await;

        // Storing loaded cannels. Extra block around to drop `known_channels` before `await`.
        {
            let mut known_channels = self.imp().channels.borrow_mut();
            to_load.extend(loaded_channels.clone());
            for channel in loaded_channels {
                known_channels.entry(channel.thread()).or_insert_with(|| {
                    log::trace!("Got a contact from the storage");
                    self.emit_by_name::<()>("channel", &[&channel]);
                    channel
                });
            }
        }

        // Note: Groups need channels to be finished initializing first due to loading participants; we cannot combine them.

        let store = self.store();
        // TODO: Error handling?
        if let Ok(groups) = tspawn!(async move { store.groups().await })
            .await
            .expect("Failed to spawn tokio")
        {
            // Construct all groups in parallel.
            let groups = groups
                .into_iter()
                .map_while(|v| v.ok())
                .map(|(key, group)| async move {
                    let revision = group.revision;
                    Channel::from_group(
                        group,
                        &GroupContextV2 {
                            master_key: Some(key.into()),
                            revision: Some(revision),
                            group_change: None,
                        },
                        self,
                    )
                    .await
                });
            let loaded_channels = futures::future::join_all(groups).await;

            // Store loaded channels. Extra block around to drop `known_channels` before `await`.
            {
                let mut known_channels = self.imp().channels.borrow_mut();
                to_load.extend(loaded_channels.clone());
                for channel in loaded_channels {
                    known_channels.entry(channel.thread()).or_insert_with(|| {
                        self.emit_by_name::<()>("channel", &[&channel]);
                        channel
                    });
                }
            }
        }

        // For each channel, load initial messages and initialize avatars, all in parallel.
        let futures = to_load.iter().map(|c| {
            futures::future::join(c.load_last(MESSAGES_INITIAL_LOAD), c.initialize_avatar())
        });
        futures::future::join_all(futures).await;

        !to_load.is_empty()
    }
}

impl Manager {
    pub(super) async fn get_group_v2(
        &self,
        master_key: [u8; 32],
    ) -> Result<Option<Group>, <StoreType as presage::store::Store>::Error> {
        log::trace!("`Manager::get_group_v2`start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.get_group_v2(master_key).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::get_group_v2`finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn retrieve_profile_by_uuid(
        &self,
        uuid: Uuid,
        profile_key: ProfileKey,
    ) -> Result<Profile, PresageError> {
        log::trace!("`Manager::retrieve_profile_by_uuid` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.retrieve_profile_by_uuid(uuid, profile_key).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::retrieve_profile_by_uuid` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn retrieve_profile_avatar_by_uuid(
        &self,
        uuid: Uuid,
        profile_key: ProfileKey,
    ) -> Result<Option<Vec<u8>>, PresageError> {
        log::trace!("`Manager::retrieve_profile_avatar_by_uuid` start");
        let internal = self.internal();
        let r = tspawn!(async move {
            internal
                .retrieve_profile_avatar_by_uuid(uuid, profile_key)
                .await
        })
        .await
        .expect("Failed to spawn tokio");
        log::trace!("`Manager::retrieve_profile_avatar_by_uuid` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn retrieve_group_avatar(
        &self,
        context: GroupContextV2,
    ) -> Result<Option<Vec<u8>>, PresageError> {
        log::trace!("`Manager::retrieve_group_avatar` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.retrieve_group_avatar(context).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::retrieve_group_avatar` finished");
        r
    }

    pub(super) async fn send_message(
        &self,
        recipient_addr: impl Into<ServiceId> + std::clone::Clone + Send + 'static,
        message: impl Into<ContentBody>,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_message` start");
        let body = message.into();
        let internal = self.internal();
        let r = tspawn!(async move {
            internal
                .send_message(recipient_addr.clone(), body.clone(), timestamp)
                .await
        })
        .await
        .expect("Failed to spawn tokio");
        log::trace!("`Manager::send_message`finished");
        Ok(r?)
    }

    pub(super) async fn send_session_reset(
        &self,
        recipient_addr: impl Into<ServiceId> + Send + 'static,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_session_reset` start");
        let internal = self.internal();
        let r =
            tspawn!(async move { internal.send_session_reset(recipient_addr, timestamp).await })
                .await
                .expect("Failed to spawn tokio");
        log::trace!("`Manager::send_session_reset` finished");
        Ok(r?)
    }

    pub(super) async fn send_message_to_group(
        &self,
        group_key: Vec<u8>,
        message: DataMessage,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_message_to_group` start");
        let internal = self.internal();
        let r = tspawn!(async move {
            internal
                .send_message_to_group(group_key, message.clone(), timestamp)
                .await
        })
        .await
        .expect("Failed to spawn tokio");
        log::trace!("`Manager::send_message_to_group` finish");
        Ok(r?)
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn get_contact_by_id(
        &self,
        id: ServiceId,
    ) -> Result<Option<presage::model::contacts::Contact>, ApplicationError> {
        log::trace!("`Manager::get_contact_by_id` start");
        let store = self.store();
        let r = tspawn!(async move { store.contact_by_id(&id).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::get_contact_by_id` finished");
        Ok(r?)
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn get_profile_key_by_id(
        &self,
        id: ServiceId,
    ) -> Result<Option<libsignal_service::prelude::ProfileKey>, ApplicationError> {
        log::trace!("`Manager::get_profile_key_by_id` start");
        let store = self.store();
        let r = tspawn!(async move { store.profile_key(&id).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::get_profile_key_by_id` finished");
        Ok(r?)
    }

    pub(super) async fn get_attachment(
        &self,
        attachment_pointer: &AttachmentPointer,
    ) -> Result<Vec<u8>, PresageError> {
        log::trace!("`Manager::get_attachment` start");
        let internal = self.internal();
        let attachment_pointer = attachment_pointer.clone();
        let r = tspawn!(async move { internal.get_attachment(&attachment_pointer).await })
            .await
            .expect("Failed to spawn tokio");
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
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, PresageError> {
        log::trace!("`Manager::upload_attachment` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.upload_attachments(attachments).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::upload_attachment` finished");
        r
    }

    pub async fn request_contacts_sync(&self) -> Result<(), ApplicationError> {
        log::trace!("`Manager::request_contacts_sync` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.request_contacts().await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::request_contacts_sync` finished");
        Ok(r?)
    }

    pub async fn link_secondary(&self, url: Url) -> Result<(), PresageError> {
        log::trace!("`Manager::link_secondary` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.link_secondary(url).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::link_secondary` finished");
        r
    }

    pub async fn unlink_secondary(&self, id: DeviceId) -> Result<(), PresageError> {
        log::trace!("`Manager::unlink_secondary` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.unlink_secondary(id).await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::unlink_secondary` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub async fn devices(&self) -> Result<Vec<DeviceInfo>, PresageError> {
        log::trace!("`Manager::devices` start");
        let internal = self.internal();
        let r = tspawn!(async move { internal.devices().await })
            .await
            .expect("Failed to spawn tokio");
        log::trace!("`Manager::devices` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub fn is_primary(&self) -> bool {
        self.imp()
            .internal
            .borrow()
            .as_ref()
            .and_then(|r| r.registration_type())
            == Some(presage::manager::RegistrationType::Primary)
    }
}

mod imp {
    use crate::prelude::*;
    use std::collections::HashMap;

    use gio::{Application, Settings, glib::DateTime};
    use glib::{BoxedAnyObject, ParamSpec, ParamSpecBoolean, Value};

    use crate::{
        backend::{Channel, Message, manager_thread::ManagerThread},
        config::BASE_ID,
    };
    use presage::store::Thread;

    pub struct Manager {
        pub(super) internal: RefCell<Option<ManagerThread>>,
        pub(super) config_store: RefCell<Option<super::StoreType>>,
        #[cfg(feature = "screenshot")]
        pub(in super::super) channels: RefCell<HashMap<Thread, Channel>>,
        #[cfg(not(feature = "screenshot"))]
        pub(super) channels: RefCell<HashMap<Thread, Channel>>,
        #[cfg(feature = "screenshot")]
        pub(in super::super) finished_setup: Cell<bool>,
        #[cfg(not(feature = "screenshot"))]
        pub(super) finished_setup: Cell<bool>,
        pub(super) last_message_datetime: RefCell<Option<DateTime>>,
        pub(super) settings: Settings,
        pub(super) application: RefCell<Option<Application>>,
    }

    impl Default for Manager {
        fn default() -> Self {
            Self {
                internal: Default::default(),
                config_store: Default::default(),
                channels: Default::default(),
                finished_setup: Default::default(),
                last_message_datetime: Default::default(),
                settings: Settings::new(BASE_ID),
                application: Default::default(),
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
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::builder("is-primary").read_only().build(),
                    ParamSpecBoolean::builder("finished-setup")
                        .read_only()
                        .build(),
                    ParamSpecBoolean::builder("last-message-datetime")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "is-primary" => self.obj().is_primary().to_value(),
                "finished-setup" => self.finished_setup.get().to_value(),
                "last-message-datetime" => self.last_message_datetime.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, _value: &Value, _pspec: &ParamSpec) {
            unimplemented!()
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![
                    Signal::builder("message")
                        .param_types([Message::static_type()])
                        .build(),
                    Signal::builder("channel")
                        .param_types([Channel::static_type()])
                        .build(),
                    Signal::builder("setup-result")
                        .param_types([BoxedAnyObject::static_type()])
                        .build(),
                    Signal::builder("error")
                        .param_types([BoxedAnyObject::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
}
