use futures::select;
use futures::FutureExt;
use futures::StreamExt;
use libsignal_service::content::ContentBody;
use libsignal_service::groups_v2::Group;
use libsignal_service::models::Contact;
use libsignal_service::prelude::Content;
use libsignal_service::prelude::GroupMasterKey;
use libsignal_service::prelude::Uuid;
use libsignal_service::proto::AttachmentPointer;
use libsignal_service::proto::DataMessage;
use libsignal_service::sender::AttachmentSpec;
use libsignal_service::sender::AttachmentUploadError;
use libsignal_service::ServiceAddress;
use presage::ConfigStore;
use presage::Manager;
use presage::Registered;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use presage::Error;

const MESSAGE_BOUND: usize = 10;

enum Command {
    RequestContactsSync(oneshot::Sender<Result<(), Error>>),
    Uuid(oneshot::Sender<Uuid>),
    GetContacts(oneshot::Sender<Result<Vec<Contact>, Error>>),
    GetGroupV2(GroupMasterKey, oneshot::Sender<Result<Group, Error>>),
    SendMessage(
        ServiceAddress,
        ContentBody,
        u64,
        oneshot::Sender<Result<(), Error>>,
    ),
    SendMessageToGroup(
        Vec<ServiceAddress>,
        DataMessage,
        u64,
        oneshot::Sender<Result<(), Error>>,
    ),
    GetAttachment(AttachmentPointer, oneshot::Sender<Result<Vec<u8>, Error>>),
    UploadAttachments(
        Vec<(AttachmentSpec, Vec<u8>)>,
        oneshot::Sender<Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, Error>>,
    ),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub struct ManagerThread {
    command_sender: mpsc::Sender<Command>,
    uuid: Uuid,
    contacts: Vec<Contact>,
}

impl Clone for ManagerThread {
    fn clone(&self) -> Self {
        Self {
            command_sender: self.command_sender.clone(),
            uuid: self.uuid.clone(),
            contacts: self
                .contacts
                .iter()
                .map(|c| almost_clone_contact(c))
                .collect(),
        }
    }
}

impl ManagerThread {
    pub async fn new<C>(
        config_store: C,
        link_callback: futures::channel::oneshot::Sender<url::Url>,
        error_callback: futures::channel::oneshot::Sender<Error>,
        content: mpsc::Sender<Content>,
        error: mpsc::Sender<Error>,
    ) -> Self
    where
        C: presage::ConfigStore + std::marker::Send + std::marker::Sync + 'static,
    {
        let (sender, receiver) = mpsc::channel(MESSAGE_BOUND);
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .expect("Failed to setup runtime")
                .block_on(async move {
                    let setup = setup_manager(config_store, link_callback).await;
                    if let Ok(manager) = setup {
                        drop(error_callback);
                        command_loop(&manager, receiver, content, error).await;
                    } else {
                        error_callback
                            .send(setup.err().unwrap())
                            .expect("Failed to send error")
                    }
                });
        });

        let (sender_uuid, receiver_uuid) = oneshot::channel();
        sender
            .send(Command::Uuid(sender_uuid))
            .await
            .expect("Command sending failed");
        let uuid = receiver_uuid.await.expect("Callback receiving failed");

        let (sender_contacts, receiver_contacts) = oneshot::channel();
        sender
            .send(Command::GetContacts(sender_contacts))
            .await
            .expect("Command sending failed");
        let contacts = receiver_contacts.await.expect("Callback receiving failed");
        if let Err(_e) = &contacts {
            // TODO: Error handling
            log::error!("Could not load contacts");
        }
        Self {
            command_sender: sender,
            uuid,
            contacts: contacts.unwrap_or_default(),
        }
    }
}

impl ManagerThread {
    pub async fn request_contacts_sync(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RequestContactsSync(sender))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn get_contacts(&self) -> Result<impl Iterator<Item = Contact> + '_, Error> {
        Ok(self.contacts.iter().map(|c| almost_clone_contact(c)))
    }

    pub fn get_contact_by_id(&self, id: Uuid) -> Result<Option<Contact>, Error> {
        Ok(self
            .contacts
            .iter()
            .filter(|c| c.address.uuid == Some(id))
            .map(|c| almost_clone_contact(c))
            .next())
    }

    pub async fn get_group_v2(&self, group_master_key: GroupMasterKey) -> Result<Group, Error> {
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
                message.into(),
                timestamp,
                sender,
            ))
            .await
            .expect("Command sending failed");
        receiver.await.expect("Callback receiving failed")
    }

    pub async fn send_message_to_group(
        &self,
        recipients: impl IntoIterator<Item = ServiceAddress>,
        message: DataMessage,
        timestamp: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SendMessageToGroup(
                recipients.into_iter().collect(),
                message.into(),
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
}

async fn setup_manager<C>(
    config_store: C,
    link_callback: futures::channel::oneshot::Sender<url::Url>,
) -> Result<presage::Manager<C, presage::Registered>, Error>
where
    C: ConfigStore + 'static,
{
    let man = if let Ok(manager) = presage::Manager::load_registered(config_store.clone()) {
        log::debug!("The configuration store is already valid, loading a registered account");
        drop(link_callback);
        Ok(manager)
    } else {
        log::debug!("The config store is not valid yet, linking with a secondary device");
        presage::Manager::link_secondary_device(
            config_store.clone(),
            presage::prelude::SignalServers::Production,
            "flare".to_string(),
            link_callback,
        )
        .await
    };
    man
}

async fn command_loop<C: ConfigStore + 'static>(
    manager: &Manager<C, Registered>,
    mut receiver: mpsc::Receiver<Command>,
    content: mpsc::Sender<Content>,
    error: mpsc::Sender<Error>,
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
                                if let Err(_) = content.send(msg).await {
                                    break 'outer;
                                }
                            } else {
                                break;
                            }
                        },
                        cmd = receiver.recv().fuse() => {
                            if let Some(cmd) = cmd {
                                handle_command(manager, cmd).await;
                            }
                        },
                        complete => break,
                    }
                }
            }
            Err(e) => {
                error.send(e).await.expect("Callback sending failed");
                break;
            }
        }
        log::debug!("Websocket closed, trying again");
    }
}

async fn handle_command<C: ConfigStore + 'static>(
    manager: &Manager<C, Registered>,
    command: Command,
) {
    match command {
        Command::RequestContactsSync(callback) => callback
            .send(manager.request_contacts_sync().await)
            .expect("Callback sending failed"),
        Command::Uuid(callback) => callback
            .send(manager.uuid())
            .expect("Callback sending failed"),
        Command::GetContacts(callback) => callback
            .send(manager.get_contacts().map(|c| c.collect()))
            .expect("Callback sending failed"),
        Command::GetGroupV2(master_key, callback) => callback
            .send(manager.get_group_v2(master_key).await)
            .map_err(|_| ())
            .expect("Callback sending failed"),
        Command::SendMessage(recipient_address, message, timestamp, callback) => callback
            .send(
                manager
                    .send_message(recipient_address, message, timestamp)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::SendMessageToGroup(recipients, message, timestamp, callback) => callback
            .send(
                manager
                    .send_message_to_group(recipients, message, timestamp)
                    .await,
            )
            .expect("Callback sending failed"),
        Command::GetAttachment(attachment, callback) => callback
            .send(manager.get_attachment(&attachment).await)
            .expect("Callback sending failed"),
        Command::UploadAttachments(attachments, callback) => callback
            .send(manager.upload_attachments(attachments).await)
            .expect("Callback sending failed"),
    }
}

// TODO: Clone attachment
fn almost_clone_contact(contact: &Contact) -> Contact {
    Contact {
        address: contact.address.clone(),
        name: contact.name.clone(),
        color: contact.color.clone(),
        verified: contact.verified.clone(),
        profile_key: contact.profile_key.clone(),
        blocked: contact.blocked.clone(),
        expire_timer: contact.expire_timer.clone(),
        inbox_position: contact.inbox_position.clone(),
        archived: contact.archived.clone(),
        avatar: None,
    }
}
