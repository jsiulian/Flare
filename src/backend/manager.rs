use crate::prelude::*;

use std::{collections::HashMap, io::Write, ops::Bound, path::Path, time::Duration};

use gio::Application;
use gio::Settings;
use libsignal_service::{
    content::ContentBody,
    groups_v2::Group,
    proto::{AttachmentPointer, DataMessage, GroupContextV2},
    push_service::DeviceInfo,
    sender::{AttachmentSpec, AttachmentUploadError},
    Profile, ServiceAddress,
};
use oo7::Keyring;
use presage::store::{ContentsStore, StateStore, Thread};
use presage_store_sled::MigrationConflictStrategy;
use rand::distributions::DistString;
use url::Url;

use super::{manager_thread::ManagerThread, Channel, Contact, Message};
use crate::backend::message::{DisplayMessage, DisplayMessageExt};
use crate::{dbus::Feedbackd, gspawn, tspawn, ApplicationError};

const MESSAGE_BOUND: usize = 100;
const MESSAGES_INITIAL_LOAD: usize = 10;
const INIT_CHANNELS_SLEEP_SECS: u64 = 10;
const SCHEMA_ATTRIBUTE: &str = "xdg:schema";
const ATTRIBUTE_PASSWORD: (&str, &str) = ("type", "password");
const SECRET_LENGTH: usize = 64;
const STORE_VERSION_FILE: &str = "store_version";

gtk::glib::wrapper! {
    /// The manager is the core of the logic of Flare.
    ///
    /// It is mostly a wrapper around [ManagerThread] (which is itself a wrapper around [presage::Manager]).
    /// It also has other functions, like caching the channels which are in use or sending notifications.
    pub struct Manager(ObjectSubclass<imp::Manager>);
}

type StoreType = presage_store_sled::SledStore;
type PresageError = presage::Error<presage_store_sled::SledStoreError>;

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
        let distribution = rand::distributions::Standard {};
        let secret = distribution.sample_string(&mut rand::thread_rng(), SECRET_LENGTH);
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

    let path_store_version = path.join(STORE_VERSION_FILE);
    if path.exists() && !path_store_version.exists() {
        log::info!("Migrating from old store to new store. Removing old store");
        std::fs::remove_dir_all(path)?;
    }

    let passphrase = tspawn!(async { encryption_password().await })
        .await
        .expect("Failed tokio join")?;
    let store = Ok(presage_store_sled::SledStore::open_with_passphrase(
        path,
        Some(&passphrase),
        MigrationConflictStrategy::BackupAndDrop,
        presage_store_sled::OnNewIdentity::Trust,
    )?);

    if !path_store_version.exists() {
        log::info!("Creating file to specify the new store version.");
        let mut file = std::fs::File::create(path_store_version)?;
        file.write_all(b"1")?;
    }

    store
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
        if self.imp().settings.boolean("notifications") {
            if let Some(application) = self.application() {
                log::trace!("Sending a notification");
                application.send_notification(id.as_deref(), notification);
                if let Some(feedbackd) = self.feedbackd() {
                    // Ignore errors creating feedback
                    let _ = crate::tspawn!(async move { feedbackd.feedback().await }).await;
                }
            }
        }
    }

    pub fn application(&self) -> Option<Application> {
        self.imp().application.borrow().clone()
    }

    pub fn feedbackd(&self) -> Option<Feedbackd> {
        self.imp().feedbackd.borrow().clone()
    }

    pub fn clear_registration(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing the manager");
        if let Some(config_store) = self.imp().config_store.borrow_mut().as_mut() {
            config_store.clear_registration()?;
        }
        Ok(())
    }

    pub fn clear_messages(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing messages from the manager");
        if let Some(config_store) = self.imp().config_store.borrow_mut().as_mut() {
            config_store.clear_messages()?;
        }
        Ok(())
    }

    pub fn clear_contacts(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing contacts from the manager");
        if let Some(config_store) = self.imp().config_store.borrow_mut().as_mut() {
            config_store.clear_contacts()?;
        }
        Ok(())
    }

    pub fn clear_groups(&self) -> Result<(), ApplicationError> {
        log::trace!("Clearing groups from the manager");
        if let Some(config_store) = self.imp().config_store.borrow_mut().as_mut() {
            config_store.clear_groups()?;
        }
        Ok(())
    }

    pub fn clear_channel_messages(&self, channel: &Channel) -> Result<(), ApplicationError> {
        crate::trace!(
            "Clearing channel messages the manager for: {}",
            channel.title()
        );
        if let Some(config_store) = self.imp().config_store.borrow_mut().as_mut() {
            if let Some(thread) = channel.thread() {
                config_store.clear_thread(&thread)?;
            } else {
                log::warn!("Was asked to clear a channel without an associated thread");
            }
        }
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
        self.internal()
            .submit_recaptcha_challenge(token, captcha)
            .await?;
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
        Ok(self
            .internal()
            .messages(
                thread.clone(),
                (
                    Bound::Unbounded,
                    from.map(Bound::Excluded).unwrap_or(Bound::Unbounded),
                ),
            )
            .await?
            .rev()
            .filter_map(|o| o.ok()))
    }

    /// Asynchronously initialize the manager. This will never return unless there is an error.
    ///
    /// This includes:
    /// - Setting up the configuration store.
    /// - Constructing the manager thread, reacting to any setup results or errors that happen.
    /// - Initializing feedbackd.
    /// - Loading the stored channels.
    /// - Listening for messages or errors and propagating them to the correct channels.
    #[cfg(not(feature = "screenshot"))]
    pub async fn init<P: AsRef<Path>>(&self, p: &P) -> Result<(), ApplicationError> {
        use futures::channel::{mpsc, oneshot};
        use futures::{select, FutureExt, StreamExt};
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

        if internal.is_none() {
            if let Some(error_opt) = receive_error.next().await {
                log::error!("Got error after linking device: {}", error_opt);
                return Err(error_opt);
            }
        }

        log::trace!("Setup feedbackd");
        let feedbackd = crate::tspawn!(async { Feedbackd::new().await })
            .await
            .map(|e| e.ok())
            .ok()
            .flatten();
        if feedbackd.is_none() {
            log::info!("Feedbackd not available, there will not be feedback for notifications");
        }
        self.imp().internal.swap(&RefCell::new(internal));
        self.imp().feedbackd.swap(&RefCell::new(feedbackd));

        // Check again if is primary, after setup is successful.
        self.notify("is-primary");

        let mut channels_init = self.init_channels().await;

        crate::info!("Own uuid: {:?}", self.uuid());
        log::debug!("Start receiving messages");
        'outer: loop {
            // On setup, it takes a while for channels to sync. Therefore try multiple times until there are channels.
            let mut init_channels_sleep =
                gtk::glib::timeout_future(Duration::from_secs(INIT_CHANNELS_SLEEP_SECS)).fuse();
            select! {
                () = &mut init_channels_sleep => {
                    if !channels_init {
                        channels_init = self.init_channels().await;
                    }
                }
                // Receive errors.
                error_opt = receive_error.next().fuse() => {
                    if error_opt.is_none() {
                        break 'outer;
                    }
                    return Err(error_opt.unwrap());
                }
                // Receive messages.
                msg_opt = receive_content.next().fuse() => {
                    if msg_opt.is_none() {
                        break 'outer;
                    }
                    let msg = msg_opt.unwrap();
                    let message = Message::from_content(msg, self).await;
                    if message.is_none() {
                        log::trace!("Manager ignoring empty message");
                        continue;
                    }
                    let message = message.unwrap();

                    let channel = message.channel();
                    let channel = {
                        let channels = self.imp().channels.borrow();
                        crate::debug!("Got from channel: {}", channel.property::<String>("title"));
                        if let Some(stored_channel) = channels.get(&channel.internal_hash()) {
                            log::debug!("Message from a already existing channel");
                            stored_channel.clone()
                        } else {
                            drop(channels);
                            log::debug!("Got a message from a new channel");
                            self.emit_by_name::<()>("channel", &[&channel]);
                            let mut channels_mut = self.imp().channels.borrow_mut();
                            channels_mut.insert(channel.internal_hash(), channel.clone());
                            channel
                        }
                    };
                    if channel.new_message(message).await.is_err() {
                        break 'outer;
                    }
                    log::debug!("Emitting message");
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
        uuid: Uuid,
        group: &Option<GroupContextV2>,
    ) -> Channel {
        let found = if group.is_some() {
            self.available_channels()
                .into_iter()
                .find(|c| &c.group_context() == group)
        } else {
            self.available_channels()
                .into_iter()
                .find(|c| c.uuid() == Some(uuid))
        };
        if let Some(found) = found {
            return found;
        }
        let contact = Contact::from_service_address(&ServiceAddress::new_aci(uuid), self);
        Channel::from_contact_or_group(contact, group, self).await
    }

    #[cfg(not(feature = "screenshot"))]
    pub fn list_contacts(&self) -> Vec<Contact> {
        self.store()
            .contacts()
            .map(|i| {
                i.filter_map(|c| {
                    c.ok()
                        .filter(|c| !c.archived)
                        .map(|c| Contact::from_contact(c, self))
                })
                .collect()
            })
            .unwrap_or_default()
    }

    pub fn self_contact(&self) -> Contact {
        let presage_contact = libsignal_service::models::Contact {
            uuid: self.uuid(),
            // TODO: Get own phone number?
            phone_number: None,
            name: "".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
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
        let mut to_load = vec![];

        // Construct all channels in parallel.
        let channels_futures = self.list_contacts().into_iter().map(|c| async move {
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
                log::trace!("Got a contact from the storage");
                self.emit_by_name::<()>("channel", &[&channel]);
                known_channels.insert(channel.internal_hash(), channel);
            }
        }

        // Note: Groups need channels to be finished initializing first due to loading participants; we cannot combine them.

        // TODO: Error handling?
        if let Ok(groups) = self.store().groups() {
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
                    self.emit_by_name::<()>("channel", &[&channel]);
                    known_channels.insert(channel.internal_hash(), channel);
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
        let r = self.internal().get_group_v2(master_key).await;
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
        let r = self
            .internal()
            .retrieve_profile_by_uuid(uuid, profile_key)
            .await;
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
        let r = self
            .internal()
            .retrieve_profile_avatar_by_uuid(uuid, profile_key)
            .await;
        log::trace!("`Manager::retrieve_profile_avatar_by_uuid` finished");
        r
    }

    #[cfg(not(feature = "screenshot"))]
    pub(super) async fn retrieve_group_avatar(
        &self,
        context: GroupContextV2,
    ) -> Result<Option<Vec<u8>>, PresageError> {
        log::trace!("`Manager::retrieve_group_avatar` start");
        let r = self.internal().retrieve_group_avatar(context).await;
        log::trace!("`Manager::retrieve_group_avatar` finished");
        r
    }

    pub(super) async fn send_message(
        &self,
        recipient_addr: impl Into<ServiceAddress> + std::clone::Clone,
        message: impl Into<ContentBody>,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_message` start");
        let body = message.into();
        let r = self
            .internal()
            .send_message(recipient_addr.clone(), body.clone(), timestamp)
            .await;
        log::trace!("`Manager::send_message`finished");
        Ok(r?)
    }

    pub(super) async fn send_session_reset(
        &self,
        recipient_addr: impl Into<ServiceAddress>,
        timestamp: u64,
    ) -> Result<(), ApplicationError> {
        log::trace!("`Manager::send_session_reset` start");
        let r = self
            .internal()
            .send_session_reset(recipient_addr, timestamp)
            .await;
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
        let r = self
            .internal()
            .send_message_to_group(group_key, message.clone(), timestamp)
            .await;
        log::trace!("`Manager::send_message_to_group` finish");
        Ok(r?)
    }

    pub(super) fn get_contact_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<libsignal_service::models::Contact>, ApplicationError> {
        log::trace!("`Manager::get_contact_by_id` start");
        let r = self.store().contact_by_id(&id);
        log::trace!("`Manager::get_contact_by_id` finished");
        Ok(r?)
    }

    pub(super) async fn get_attachment(
        &self,
        attachment_pointer: &AttachmentPointer,
    ) -> Result<Vec<u8>, PresageError> {
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
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, PresageError> {
        log::trace!("`Manager::upload_attachment` start");
        let r = self.internal().upload_attachments(attachments).await;
        log::trace!("`Manager::upload_attachment` finished");
        r
    }

    pub async fn request_contacts_sync(&self) -> Result<(), ApplicationError> {
        log::trace!("`Manager::request_contacts_sync` start");
        let r = self.internal().request_contacts().await;
        log::trace!("`Manager::request_contacts_sync` finished");
        Ok(r?)
    }

    pub async fn link_secondary(&self, url: Url) -> Result<(), PresageError> {
        log::trace!("`Manager::link_secondary` start");
        let r = self.internal().link_secondary(url).await;
        log::trace!("`Manager::link_secondary` finished");
        r
    }

    pub async fn unlink_secondary(&self, id: i64) -> Result<(), PresageError> {
        log::trace!("`Manager::unlink_secondary` start");
        let r = self.internal().unlink_secondary(id).await;
        log::trace!("`Manager::unlink_secondary` finished");
        r
    }

    pub async fn devices(&self) -> Result<Vec<DeviceInfo>, PresageError> {
        log::trace!("`Manager::devices` start");
        let r = self.internal().devices().await;
        log::trace!("`Manager::devices` finished");
        r
    }
}

mod imp {
    use crate::prelude::*;
    use std::collections::HashMap;

    use gio::{Application, Settings};
    use glib::{BoxedAnyObject, ParamSpec, ParamSpecBoolean, Value};

    use crate::dbus::Feedbackd;
    use crate::{
        backend::{manager_thread::ManagerThread, Channel, Message},
        config::BASE_ID,
    };

    pub struct Manager {
        pub(super) internal: RefCell<Option<ManagerThread>>,
        pub(super) config_store: RefCell<Option<super::StoreType>>,
        #[cfg(feature = "screenshot")]
        pub(in super::super) channels: RefCell<HashMap<u64, Channel>>,
        #[cfg(not(feature = "screenshot"))]
        pub(super) channels: RefCell<HashMap<u64, Channel>>,
        pub(super) settings: Settings,
        pub(super) application: RefCell<Option<Application>>,

        pub(super) feedbackd: RefCell<Option<Feedbackd>>,
    }

    impl Default for Manager {
        fn default() -> Self {
            Self {
                internal: Default::default(),
                config_store: Default::default(),
                channels: Default::default(),
                settings: Settings::new(BASE_ID),
                application: Default::default(),
                feedbackd: Default::default(),
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
            static PROPERTIES: Lazy<Vec<ParamSpec>> =
                Lazy::new(|| vec![ParamSpecBoolean::builder("is-primary").read_only().build()]);
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "is-primary" => (self
                    .internal
                    .borrow()
                    .as_ref()
                    .and_then(|r| r.registration_type())
                    == Some(presage::manager::RegistrationType::Primary))
                .to_value(),
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
                ]
            });
            SIGNALS.as_ref()
        }
    }
}
