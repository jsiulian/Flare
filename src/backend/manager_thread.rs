//! The [ManagerThread] is a [presage::Manager] running on its own tokio thread.
//!
//! This is required as the tokio and glib runtimes do not play together nicely (i.e. this can lead to crashes).
//! Furthermore, one cannot just send [presage::Manager] to a separate tokio thread and send back results each time one
//! runs a method on it as [presage::Manager] is `!Sync`. Therefore, this workaround exists which runs the manager in a
//! separate thread and communication between threads happens via channels. Those communications are two-way,
//! [crate::backend::Manger] to [ManagerThread] happens via sending a [Command] over a channel, this command includes a
//! channel back where the result is sent. This way is also accompanied by methods of [ManagerThread] that are similar
//! to those of [presage::Manager]. The other way is with a separate channel which has to be given to [ManagerThread]
//! when it is constructed.

use std::{cell::OnceCell, ops::Bound};

use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, SinkExt, StreamExt, TryFutureExt, join, select};
use libsignal_service::{
    Profile,
    configuration::SignalServers,
    content::ContentBody,
    prelude::{Content, ProfileKey, Uuid, phonenumber},
    proto::{AttachmentPointer, DataMessage, GroupContextV2},
    protocol::ServiceId,
    push_service::DeviceInfo,
    sender::{AttachmentSpec, AttachmentUploadError},
};
use presage::model::messages::Received;
use presage::{
    Manager,
    manager::{Registered, RegistrationOptions, RegistrationType},
    model::groups::Group,
    store::{ContentsStore, Thread},
};
use presage_store_sled::SledStore as Store;
use url::Url;

use crate::ApplicationError;
use crate::dbus::Login1;

const MESSAGE_BOUND: usize = 10;
const OFFLINE_SLEEP_TIMEOUT: u64 = 15;

type Error = presage::Error<<Store as presage::store::Store>::Error>;

// TODO: Reconsider ignoring in the future, but probably does not make any huge difference.
#[allow(clippy::large_enum_variant)]
enum Command {
    Uuid(oneshot::Sender<Uuid>),
    SubmitRecaptchaChallenge(String, String, oneshot::Sender<Result<(), Error>>),
    RetrieveProfileByUuid(Uuid, ProfileKey, oneshot::Sender<Result<Profile, Error>>),
    RetrieveProfile(oneshot::Sender<Result<Profile, Error>>),
    GetGroupV2(
        [u8; 32],
        oneshot::Sender<Result<Option<Group>, <Store as presage::store::Store>::Error>>,
    ),
    SendSessionReset(ServiceId, u64, oneshot::Sender<Result<(), Error>>),
    SendMessage(
        ServiceId,
        Box<ContentBody>,
        u64,
        oneshot::Sender<Result<(), Error>>,
    ),
    SendMessageToGroup(
        Vec<u8>,
        Box<DataMessage>,
        u64,
        oneshot::Sender<Result<(), Error>>,
    ),
    GetAttachment(AttachmentPointer, oneshot::Sender<Result<Vec<u8>, Error>>),
    UploadAttachments(
        Vec<(AttachmentSpec, Vec<u8>)>,
        oneshot::Sender<Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, Error>>,
    ),
    Messages(
        Thread,
        (Bound<u64>, Bound<u64>),
        oneshot::Sender<
            Result<
                <presage_store_sled::SledStore as presage::store::ContentsStore>::MessagesIter,
                Error,
            >,
        >,
    ),
    RegistrationType(oneshot::Sender<RegistrationType>),
    LinkSecondary(Url, oneshot::Sender<Result<(), Error>>),
    UnlinkSecondary(i64, oneshot::Sender<Result<(), Error>>),
    Devices(oneshot::Sender<Result<Vec<DeviceInfo>, Error>>),
    RequestContacts(oneshot::Sender<Result<(), Error>>),
    RetrieveProfileAvatarByUuid(
        Uuid,
        ProfileKey,
        oneshot::Sender<Result<Option<Vec<u8>>, Error>>,
    ),
    RetrieveGroupAvatar(
        GroupContextV2,
        oneshot::Sender<Result<Option<Vec<u8>>, Error>>,
    ),
}

/// Message from the UI to the manager thread how to do setup.
#[derive(Debug)]
pub enum SetupDecision {
    /// Server, Device Name
    Link(SignalServers, String),
    /// Server, Phone Number, Captcha
    Register(SignalServers, phonenumber::PhoneNumber, String),
}

pub type SetupConfirmation = String;

/// Message from the manager thread to the UI on what steps need to be taken for setting up the device.
#[derive(Debug)]
pub enum SetupResult {
    /// Setup must make a decision. Either register as primary device or link device.
    Pending(OnceCell<futures::channel::oneshot::Sender<SetupDecision>>),
    /// The manager is pending a SMS confirmation code.
    Confirm(OnceCell<futures::channel::oneshot::Sender<SetupConfirmation>>),
    /// Display QR code to link.
    DisplayLinkQR(Url),
    /// Everything is finished
    Finished,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uuid(_) => f.debug_tuple("Uuid").field(&format_args!("_")).finish(),
            _ => f.write_str("Unknown"),
        }
    }
}

pub struct ManagerThread {
    /// Sending commands to the thread.
    command_sender: mpsc::Sender<Command>,
    /// Cache for how the device is registered (primary or secondary).
    registration_type: Option<RegistrationType>,
    /// Cache for the own UUID.
    uuid: Uuid,
    /// Cache for the own profile.
    profile: Option<Profile>,
}

impl Clone for ManagerThread {
    fn clone(&self) -> Self {
        Self {
            command_sender: self.command_sender.clone(),
            registration_type: self.registration_type.clone(),
            uuid: self.uuid,
            profile: self.profile.clone(),
        }
    }
}

impl ManagerThread {
    /// Construct a new [ManagerThread].
    ///
    /// The parameters are as follows:
    ///
    /// - `config_store`: The [Store] to store data to.
    /// - `setup_callback`: If setup is required, the required steps will be sent over this channel.
    /// - `error_callback`: If an error during setup happens, it will be sent through this channel. If no error happened, the channel will be closed.
    /// - `content`: The messages received by the thread.
    /// - `error`: Any errors receiving messages.
    pub async fn new(
        config_store: Store,
        setup_callback: futures::channel::mpsc::Sender<SetupResult>,
        error_callback: futures::channel::oneshot::Sender<Error>,
        content: mpsc::UnboundedSender<Content>,
        error: mpsc::Sender<ApplicationError>,
    ) -> Option<Self> {
        let (mut sender, receiver) = mpsc::channel(MESSAGE_BOUND);
        let thread = std::thread::Builder::new()
            .name("ManagerThread".into())
            // Note: Increased stack size required, otherwise this thread can use too much of it and crash Flare.
            .stack_size(8 * 1024 * 1024);
        let _ = thread.spawn(move || {
            let mut error_clone = error.clone();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::TOKIO_RUNTIME.block_on(async move {
                    // XXX: Make sure the initial sync is finished (requires upstream presage changes).
                    // See https://github.com/whisperfish/presage/pull/212.
                    let manager_receive = setup_manager(config_store, setup_callback).await;
                    if let Ok(mut manager_receive) = manager_receive {
                        log::trace!("Starting command loop");
                        drop(error_callback);
                        command_loop(&mut manager_receive, receiver, content, error).await;
                    } else {
                        let e = manager_receive.err().unwrap();
                        log::trace!("Got error: {}", e);
                        error_callback.send(e).expect("Failed to send error")
                    }
                });
            }));
            if let Err(_e) = panic {
                log::error!("Manager-thread paniced");
                tokio::runtime::Runtime::new()
                    .expect("Failed to setup runtime")
                    .block_on(async move {
                        error_clone
                            .send(ApplicationError::ManagerThreadPanic)
                            .await
                            .expect("Failed to send error");
                    });
            }
        });

        // Fill cache

        let (sender_uuid, receiver_uuid) = oneshot::channel();
        if sender.send(Command::Uuid(sender_uuid)).await.is_err() {
            return None;
        }
        let uuid = receiver_uuid.await;

        if uuid.is_err() {
            return None;
        }

        let (sender_registration_type, receiver_registration_type) = oneshot::channel();
        if sender
            .send(Command::RegistrationType(sender_registration_type))
            .await
            .is_err()
        {
            return None;
        }
        let registration_type = receiver_registration_type.await;

        if registration_type.is_err() {
            return None;
        }

        let (sender_profile, receiver_profile) = oneshot::channel();
        if sender
            .send(Command::RetrieveProfile(sender_profile))
            .await
            .is_err()
        {
            return None;
        }
        let profile = receiver_profile.await;

        if profile.is_err() {
            return None;
        }

        Some(Self {
            command_sender: sender,
            uuid: uuid.unwrap(),
            registration_type: Some(registration_type.unwrap()),
            profile: profile.unwrap().ok(),
        })
    }
}

// These methods mostly mimic the [presage::Manager].
impl ManagerThread {
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn registration_type(&self) -> Option<RegistrationType> {
        self.registration_type.clone()
    }

    pub async fn submit_recaptcha_challenge(
        &self,
        token: String,
        captcha: String,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::SubmitRecaptchaChallenge(token, captcha, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn retrieve_profile_by_uuid(
        &self,
        uuid: Uuid,
        profile_key: ProfileKey,
    ) -> Result<Profile, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::RetrieveProfileByUuid(uuid, profile_key, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub fn retrieve_profile(&self) -> Option<Profile> {
        self.profile.clone()
    }

    pub async fn get_group_v2(
        &self,
        group_master_key: [u8; 32],
    ) -> Result<Option<Group>, <Store as presage::store::Store>::Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::GetGroupV2(group_master_key, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn send_message(
        &self,
        recipient_addr: impl Into<ServiceId>,
        message: impl Into<ContentBody>,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::SendMessage(
                recipient_addr.into(),
                Box::new(message.into()),
                timestamp,
                sender,
            ))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn send_session_reset(
        &self,
        recipient_addr: impl Into<ServiceId>,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::SendSessionReset(
                recipient_addr.into(),
                timestamp,
                sender,
            ))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn send_message_to_group(
        &self,
        group_key: Vec<u8>,
        message: DataMessage,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::SendMessageToGroup(
                group_key,
                Box::new(message),
                timestamp,
                sender,
            ))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn get_attachment(
        &self,
        attachment_pointer: &AttachmentPointer,
    ) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::GetAttachment(attachment_pointer.clone(), sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn upload_attachments(
        &self,
        attachments: Vec<(AttachmentSpec, Vec<u8>)>,
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::UploadAttachments(attachments, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn messages(
        &self,
        thread: Thread,
        range: (Bound<u64>, Bound<u64>),
    ) -> Result<<presage_store_sled::SledStore as presage::store::ContentsStore>::MessagesIter, Error>
    {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::Messages(thread, range, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn link_secondary(&self, url: Url) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::LinkSecondary(url, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn unlink_secondary(&self, id: i64) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::UnlinkSecondary(id, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn devices(&self) -> Result<Vec<DeviceInfo>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::Devices(sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn request_contacts(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::RequestContacts(sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn retrieve_profile_avatar_by_uuid(
        &self,
        uuid: Uuid,
        profile_key: ProfileKey,
    ) -> Result<Option<Vec<u8>>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::RetrieveProfileAvatarByUuid(
                uuid,
                profile_key,
                sender,
            ))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn retrieve_group_avatar(
        &self,
        context: GroupContextV2,
    ) -> Result<Option<Vec<u8>>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .clone()
            .send(Command::RetrieveGroupAvatar(context, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }
}

/// Setting up the manager if required.
async fn setup_manager(
    config_store: Store,
    mut setup_sender: futures::channel::mpsc::Sender<SetupResult>,
) -> Result<presage::Manager<Store, Registered>, Error> {
    if let Ok(manager) = presage::Manager::load_registered(config_store.clone()).await {
        log::debug!("The configuration store is already valid, loading a registered account");
        setup_sender
            .send(SetupResult::Finished)
            .await
            .expect("Failed to send setup results");
        Ok(manager)
    } else {
        log::debug!("The config store is not valid yet, sending decision via channel.");

        let (tx_decision, rx_decision) = futures::channel::oneshot::channel();
        let to_send = OnceCell::new();
        let _ = to_send.set(tx_decision);
        setup_sender
            .send(SetupResult::Pending(to_send))
            .await
            .expect("Failed to send setup results");
        match rx_decision.await.expect("Callback receiving failed") {
            SetupDecision::Link(servers, name) => {
                let (tx_link, rx_link) = futures::channel::oneshot::channel();
                let (_, mut manager) = join!(
                    async {
                        let link = rx_link.await.expect("Failed to receive link callback");
                        setup_sender
                            .send(SetupResult::DisplayLinkQR(link))
                            .await
                            .expect("Failed to send setup results");
                    },
                    presage::Manager::link_secondary_device(
                        config_store.clone(),
                        servers,
                        name,
                        tx_link,
                    )
                );
                // Request contact sync directly after linking.
                if let Ok(manager) = &mut manager {
                    if let Err(e) = manager.request_contacts().await {
                        log::error!("Failed to sync contacts after linking: {}", e);
                    }
                }
                setup_sender
                    .send(SetupResult::Finished)
                    .await
                    .expect("Failed to send setup results");
                manager
            }
            SetupDecision::Register(servers, phonenumber, captcha) => {
                let (tx_confirm, rx_confirm) = futures::channel::oneshot::channel();
                let manager = presage::Manager::register(
                    config_store.clone(),
                    RegistrationOptions {
                        signal_servers: servers,
                        phone_number: phonenumber,
                        use_voice_call: false,
                        captcha: Some(&captcha[..]),
                        force: false,
                    },
                )
                .await?;
                // TODO: Error handling?
                setup_sender
                    .send(SetupResult::Confirm(tx_confirm.into()))
                    .await
                    .expect("Failed to send setup results");
                let confirmation = rx_confirm
                    .await
                    .expect("Failed to receive confirm callback");
                let manager = manager.confirm_verification_code(&confirmation).await;
                setup_sender
                    .send(SetupResult::Finished)
                    .await
                    .expect("Failed to send setup results");
                manager
            }
        }
    }
}

/// The command loop where messages are received and commands from the UI thread are executed.
async fn command_loop(
    manager: &mut Manager<Store, Registered>,
    mut receiver: mpsc::Receiver<Command>,
    mut content: mpsc::UnboundedSender<Content>,
    mut error: mpsc::Sender<ApplicationError>,
) {
    'outer: loop {
        let msgs: Result<_, presage::Error<<Store as presage::store::Store>::Error>> =
            manager.receive_messages().await;
        let login1 = Login1::new().await;
        match msgs {
            Ok(messages) => {
                futures::pin_mut!(messages);
                let mut next_msg = messages.next().fuse();
                let mut await_suspend_online = login1
                    .as_ref()
                    .map(|l| {
                        l.await_suspend_wakeup()
                            .and_then(|_| async {
                                crate::utils::await_online().await;
                                Ok(())
                            })
                            .boxed()
                    })
                    .unwrap_or(futures::future::pending().boxed())
                    .fuse();
                loop {
                    select! {
                        // Receiving a message.
                        msg = next_msg => {
                            if let Some(msg) = msg {
                                if let Received::Content(msg) = msg {
                                    if content.send(*msg).await.is_err() {
                                        log::info!("Failed to send message to `Manager`, exiting");
                                        break 'outer;
                                    }
                                }
                            } else {
                                log::error!("Message stream finished. Restarting command loop.");
                                break;
                            }
                            next_msg = messages.next().fuse();
                        },
                        // Receiving a command.
                        cmd = receiver.next() => {
                            if let Some(cmd) = cmd {
                                handle_command(manager, cmd).await;
                            }
                        },
                        // The network status changed; restart the loop to restart the signal websockets.
                        _ = await_suspend_online  => {
                            log::trace!("Waking up from suspend. Restarting command loop.");
                            break;
                        },
                        complete => {
                            log::trace!("Command loop complete. Restarting command loop.");
                            break
                        },
                    }
                }
            }
            Err(e) => {
                log::error!("Got error receiving: {}, {:?}", e, e);
                let e = e.into();
                // Don't send no-internet errors, Flare is able to handle them automatically.
                if !matches!(e, ApplicationError::NoInternet) {
                    error.send(e).await.expect("Callback sending failed");
                }

                // Handle all commands while sleeping; this ensures Flare can start up without internet.
                loop {
                    select! {
                        cmd = receiver.next().fuse() => {
                            if let Some(cmd) = cmd {
                                handle_command(manager, cmd).await;
                            }
                        },
                        _ = tokio::time::sleep(std::time::Duration::from_secs(OFFLINE_SLEEP_TIMEOUT)).fuse() => {
                            break;
                        }
                    }
                }
            }
        }
        log::debug!("Websocket closed, trying again");
    }
    log::info!("Exiting `ManagerThread::command_loop`");
}

async fn handle_command(manager: &mut Manager<Store, Registered>, command: Command) {
    log::trace!("Got command: {:#?}", command);
    match command {
        // XXX: Uuid should not be used anymore.
        // XXX: Don't use nil.
        Command::Uuid(callback) => callback
            .send(manager.registration_data().service_ids.aci().into())
            .expect("Callback sending failed"),
        Command::SubmitRecaptchaChallenge(token, captcha, callback) => callback
            .send(manager.submit_recaptcha_challenge(&token, &captcha).await)
            .expect("Callback sending failed"),
        Command::RetrieveProfileByUuid(uuid, profile_key, callback) => callback
            .send(manager.retrieve_profile_by_uuid(uuid, profile_key).await)
            .expect("Callback sending failed"),
        Command::RetrieveProfile(callback) => callback
            .send(manager.retrieve_profile().await)
            .expect("Callback sending failed"),
        Command::GetGroupV2(master_key, callback) => callback
            .send(manager.store().group(master_key).await)
            .map_err(|_| ())
            .expect("Callback sending failed"),
        Command::SendSessionReset(recipient_address, timestamp, callback) => callback
            .send(
                manager
                    .send_session_reset(&recipient_address, timestamp)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::SendMessage(recipient_address, message, timestamp, callback) => callback
            .send(
                manager
                    .send_message(recipient_address, *message, timestamp)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::SendMessageToGroup(group_key, message, timestamp, callback) => callback
            .send(
                manager
                    .send_message_to_group(&group_key[..], *message, timestamp)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::GetAttachment(attachment, callback) => callback
            .send(manager.get_attachment(&attachment).await)
            .expect("Callback sending failed"),
        Command::UploadAttachments(attachments, callback) => callback
            .send(manager.upload_attachments(attachments).await)
            .expect("Callback sending failed"),
        Command::Messages(thread, range, callback) => {
            // XXX: Cannot format iterator.
            let _ = callback.send(
                manager
                    .store()
                    .messages(&thread, range)
                    .await
                    .map_err(presage::Error::Store),
            );
        }
        Command::RegistrationType(callback) => callback
            .send(manager.registration_type())
            .expect("Callback sending failed"),
        Command::LinkSecondary(url, callback) => callback
            .send(manager.link_secondary(url).await)
            .expect("Callback sending failed"),
        Command::UnlinkSecondary(id, callback) => callback
            .send(manager.unlink_secondary(id).await)
            .expect("Callback sending failed"),
        Command::Devices(callback) => callback
            .send(manager.devices().await)
            .expect("Callback sending failed"),
        Command::RequestContacts(callback) => callback
            .send(manager.request_contacts().await)
            .expect("Callback sending failed"),
        Command::RetrieveProfileAvatarByUuid(uuid, profile_key, callback) => callback
            .send(
                manager
                    .retrieve_profile_avatar_by_uuid(uuid, profile_key)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::RetrieveGroupAvatar(context, callback) => callback
            .send(manager.retrieve_group_avatar(context).await)
            .expect("Callback sending failed"),
    }
}
