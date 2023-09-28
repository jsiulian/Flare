use std::ops::Bound;

use futures::{select, FutureExt, StreamExt};
use libsignal_service::{
    groups_v2::Group, prelude::ProfileKey, sender::AttachmentUploadError, Profile,
};
use presage::{
    prelude::{content::*, AttachmentSpec, ContentBody, DataMessage, ServiceAddress, *},
    Manager, Registered, Thread,
};
use presage_store_sled::SledStore as Store;
use tokio::sync::{mpsc, oneshot};

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
    RequestContactsSync(oneshot::Sender<Result<(), Error>>),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub struct ManagerThread {
    command_sender: mpsc::Sender<Command>,
    uuid: Uuid,
    profile: Option<Profile>,
}

impl Clone for ManagerThread {
    fn clone(&self) -> Self {
        Self {
            command_sender: self.command_sender.clone(),
            uuid: self.uuid,
            profile: self.profile.clone(),
        }
    }
}

impl ManagerThread {
    pub async fn new(
        config_store: Store,
        device_name: String,
        link_callback: futures::channel::oneshot::Sender<url::Url>,
        error_callback: futures::channel::oneshot::Sender<Error>,
        content: mpsc::UnboundedSender<Content>,
        error: mpsc::Sender<ApplicationError>,
    ) -> Option<Self> {
        let (sender, receiver) = mpsc::channel(MESSAGE_BOUND);
        std::thread::spawn(move || {
            let error_clone = error.clone();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::TOKIO_RUNTIME.block_on(async move {
                    let setup = setup_manager(config_store, device_name, link_callback).await;
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
            profile: profile.unwrap().ok(),
        })
    }
}

impl ManagerThread {
    pub fn uuid(&self) -> Uuid {
        self.uuid
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

    pub async fn request_contacts_sync(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RequestContactsSync(sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }
}

async fn setup_manager(
    config_store: Store,
    name: String,
    link_callback: futures::channel::oneshot::Sender<url::Url>,
) -> Result<presage::Manager<Store, presage::Registered>, Error> {
    if let Ok(manager) = presage::Manager::load_registered(config_store.clone()).await {
        log::debug!("The configuration store is already valid, loading a registered account");
        drop(link_callback);
        Ok(manager)
    } else {
        log::debug!("The config store is not valid yet, linking with a secondary device");
        presage::Manager::link_secondary_device(
            config_store.clone(),
            presage::prelude::SignalServers::Production,
            name,
            link_callback,
        )
        .await
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
        Command::RequestContactsSync(callback) => callback
            // .send(manager.sync_contacts().await)
            .send(Ok(()))
            .expect("Callback sending failed"),
    }
}
