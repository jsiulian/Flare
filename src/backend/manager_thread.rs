use std::{cell::OnceCell, ops::Bound};

use futures::{join, select, FutureExt, SinkExt, StreamExt};
use libsignal_service::{
    groups_v2::Group, prelude::ProfileKey, push_service::DeviceInfo, sender::AttachmentUploadError,
    Profile,
};
use presage::{
    prelude::{content::*, AttachmentSpec, ContentBody, DataMessage, ServiceAddress, *},
    Manager, Registered, RegistrationOptions, RegistrationType, Thread,
};
use presage_store_sled::SledStore as Store;
use tokio::sync::{mpsc, oneshot};
use url::Url;

use crate::ApplicationError;

const MESSAGE_BOUND: usize = 10;

type Error = presage::Error<presage_store_sled::SledStoreError>;

// TODO: Reconsider ignoring in the future, but probably does not make any huge difference.
#[allow(clippy::large_enum_variant)]
enum Command {
    Uuid(oneshot::Sender<Uuid>),
    SubmitRecaptchaChallenge(String, String, oneshot::Sender<Result<(), Error>>),
    RetrieveProfileByUuid(Uuid, ProfileKey, oneshot::Sender<Result<Profile, Error>>),
    RetrieveProfile(oneshot::Sender<Result<Profile, Error>>),
    GetGroupV2(Vec<u8>, oneshot::Sender<Result<Option<Group>, Error>>),
    SendSessionReset(ServiceAddress, u64, oneshot::Sender<Result<(), Error>>),
    SendMessage(
        ServiceAddress,
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
            Result<<presage_store_sled::SledStore as presage::Store>::MessagesIter, Error>,
        >,
    ),
    RegistrationType(oneshot::Sender<RegistrationType>),
    LinkSecondary(Url, oneshot::Sender<Result<(), Error>>),
    UnlinkSecondary(i64, oneshot::Sender<Result<(), Error>>),
    LinkedDevices(oneshot::Sender<Result<Vec<DeviceInfo>, Error>>),
}

#[derive(Debug)]
pub enum SetupDecision {
    /// Server, Device Name
    Link(SignalServers, String),
    /// Server, Phone Number, Captcha
    Register(SignalServers, phonenumber::PhoneNumber, String),
}

pub type SetupConfirmation = String;

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
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub struct ManagerThread {
    command_sender: mpsc::Sender<Command>,
    registration_type: Option<RegistrationType>,
    uuid: Uuid,
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
    pub async fn new(
        config_store: Store,
        setup_callback: futures::channel::mpsc::Sender<SetupResult>,
        error_callback: futures::channel::oneshot::Sender<Error>,
        content: mpsc::UnboundedSender<Content>,
        error: mpsc::Sender<ApplicationError>,
    ) -> Option<Self> {
        let (sender, receiver) = mpsc::channel(MESSAGE_BOUND);
        std::thread::spawn(move || {
            let error_clone = error.clone();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::TOKIO_RUNTIME.block_on(async move {
                    let setup = setup_manager(config_store, setup_callback).await;
                    if let Ok(mut manager) = setup {
                        log::trace!("Starting command loop");
                        drop(error_callback);
                        command_loop(&mut manager, receiver, content, error).await;
                    } else {
                        let e = setup.err().unwrap();
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
            .send(Command::RetrieveProfileByUuid(uuid, profile_key, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub fn retrieve_profile(&self) -> Option<Profile> {
        self.profile.clone()
    }

    pub async fn get_group_v2(&self, group_master_key: Vec<u8>) -> Result<Option<Group>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::GetGroupV2(group_master_key, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn send_message(
        &self,
        recipient_addr: impl Into<ServiceAddress>,
        message: impl Into<ContentBody>,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
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
        recipient_addr: impl Into<ServiceAddress>,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
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
            .send(Command::UploadAttachments(attachments, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn messages(
        &self,
        thread: Thread,
        range: (Bound<u64>, Bound<u64>),
    ) -> Result<<presage_store_sled::SledStore as presage::Store>::MessagesIter, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::Messages(thread, range, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn link_secondary(&self, url: Url) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::LinkSecondary(url, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn unlink_secondary(&self, id: i64) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::UnlinkSecondary(id, sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn linked_devices(&self) -> Result<Vec<DeviceInfo>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::LinkedDevices(sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }
}

async fn setup_manager(
    config_store: Store,
    mut setup_sender: futures::channel::mpsc::Sender<SetupResult>,
) -> Result<presage::Manager<Store, presage::Registered>, Error> {
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
        match rx_decision.await.ok().expect("Callback receiving failed") {
            SetupDecision::Link(servers, name) => {
                let (tx_link, rx_link) = futures::channel::oneshot::channel();
                let (_, manager) = join!(
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

async fn command_loop(
    manager: &mut Manager<Store, Registered>,
    mut receiver: mpsc::Receiver<Command>,
    content: mpsc::UnboundedSender<Content>,
    error: mpsc::Sender<ApplicationError>,
) {
    'outer: loop {
        let msgs = manager.receive_messages().await;
        match msgs {
            Ok(messages) => {
                futures::pin_mut!(messages);
                loop {
                    select! {
                        msg = messages.next().fuse() => {
                            if let Some(msg) = msg {
                                if content.send(msg).is_err() {
                                    log::info!("Failed to send message to `Manager`, exiting");
                                    break 'outer;
                                }
                            } else {
                                log::error!("Message stream finished. Restarting command loop.");
                                break;
                            }
                        },
                        cmd = receiver.recv().fuse() => {
                            if let Some(cmd) = cmd {
                                handle_command(manager, cmd).await;
                            }
                        },
                        _ = crate::utils::await_suspend_wakeup_online().fuse() => {
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
                // TODO: Think about maybe handling if the application is not in the background?
                if !matches!(e, ApplicationError::NoInternet) {
                    error.send(e).await.expect("Callback sending failed");
                }
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            }
        }
        log::debug!("Websocket closed, trying again");
    }
    log::info!("Exiting `ManagerThread::command_loop`");
}

async fn handle_command(manager: &mut Manager<Store, Registered>, command: Command) {
    log::trace!("Got command: {:?}", command);
    match command {
        // XXX: Uuid should not be used anymore.
        Command::Uuid(callback) => callback
            .send(manager.state().service_ids.aci)
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
            .send(manager.group(&master_key[..]))
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
            let _ = callback.send(manager.messages(&thread, range));
        }
        Command::RegistrationType(callback) => callback
            // .send(manager.sync_contacts().await)
            .send(manager.registration_type())
            .expect("Callback sending failed"),
        Command::LinkSecondary(url, callback) => callback
            .send(manager.link_secondary(url).await)
            .expect("Callback sending failed"),
        Command::UnlinkSecondary(id, callback) => callback
            .send(manager.unlink_secondary(id).await)
            .expect("Callback sending failed"),
        Command::LinkedDevices(callback) => callback
            .send(manager.linked_devices().await)
            .expect("Callback sending failed"),
    }
}
