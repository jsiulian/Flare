use std::cell::RefCell;

use gdk_pixbuf::{glib::Object, prelude::ObjectExt};
use gio::subclass::prelude::ObjectSubclassIsExt;
use libsignal_service::{content::Reaction, ServiceAddress};
use presage::prelude::{
    proto::{data_message::Quote, sync_message::Sent},
    Content, ContentBody, DataMessage, SyncMessage,
};

use crate::backend::{Attachment, Channel, Contact};

use super::Manager;

gtk::glib::wrapper! {
    pub struct Message(ObjectSubclass<imp::Message>);
}

impl Message {
    pub fn from_text_channel_sender<S: AsRef<str>>(
        text: S,
        channel: Channel,
        sender: Contact,
        manager: &Manager,
    ) -> Self {
        log::trace!("Trying to build a message from text");
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Message`");

        let text_owned = text.as_ref().to_owned();
        let body = if text_owned.is_empty() {
            None
        } else {
            Some(text_owned)
        };

        let message = DataMessage {
            body,
            timestamp: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis() as u64,
            ),
            ..Default::default()
        };

        s.imp().data.swap(&RefCell::new(Some(message)));
        s.imp().sender.swap(&RefCell::new(Some(sender)));
        s.imp().channel.swap(&RefCell::new(Some(channel)));
        s
    }

    async fn init_data(
        &self,
        message: &DataMessage,
        contact: Contact,
        channel: Channel,
        manager: &Manager,
    ) {
        let obj = self.imp();
        obj.data.swap(&RefCell::new(Some(message.clone())));
        obj.sender.swap(&RefCell::new(Some(contact)));
        obj.channel.swap(&RefCell::new(Some(channel)));
        obj.reaction.swap(&RefCell::new(message.reaction.clone()));
        let mut attachments = Vec::with_capacity(message.attachments.len());
        for pointer in &message.attachments {
            let att = Attachment::from_pointer(pointer, manager).await;
            attachments.push(att);
        }
        obj.attachments.swap(&RefCell::new(attachments));
    }

    pub(super) async fn from_content(content: Content, manager: &Manager) -> Self {
        log::trace!("Trying to build a message from content");
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Message`");
        let metadata = &content.metadata;
        let body = &content.body;

        match body {
            ContentBody::DataMessage(message) => {
                let contact = Contact::from_service_address(&metadata.sender, manager);
                let channel =
                    Channel::from_contact_or_group(contact.clone(), &message.group_v2, manager)
                        .await;
                s.init_data(message, contact, channel, manager).await;
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(Sent {
                        destination_e164: e164,
                        destination_uuid: uuid,
                        message: Some(message),
                        ..
                    }),
                ..
            }) => {
                let contact = Contact::from_service_address(&metadata.sender, manager);
                let destination_contact = if e164.is_some() || uuid.is_some() {
                    let destination_address = ServiceAddress {
                        uuid: uuid.clone().and_then(|u| u.parse().ok()),
                        phonenumber: e164.clone().and_then(|e| e.parse().ok()),
                        relay: None,
                    };
                    Contact::from_service_address(&destination_address, manager)
                } else {
                    contact.clone()
                };
                let channel = Channel::from_contact_or_group(
                    destination_contact.clone(),
                    &message.group_v2,
                    manager,
                )
                .await;
                s.init_data(message, contact, channel, manager).await;
            }
            ContentBody::SynchronizeMessage(SyncMessage { read: read_arr, .. })
                if !read_arr.is_empty() =>
            {
                log::trace!("Got currently unhandled read-message");
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                viewed: viewed_arr, ..
            }) if !viewed_arr.is_empty() => {
                log::trace!("Got currently unhandled viewed-message");
            }
            ContentBody::CallMessage(_) => {
                log::trace!("Got currently unhandled call-message");
            }
            ContentBody::TypingMessage(_) => {
                log::trace!("Got currently unhandled call-message");
            }
            ContentBody::ReceiptMessage(_) => {
                log::trace!("Got currently unhandled receipt-message");
            }
            _ => {
                log::warn!("Do not know what to do with the message: {:?}", content);
            }
        }
        s
    }

    pub fn channel(&self) -> Option<Channel> {
        self.imp().channel.borrow().clone()
    }

    pub(super) fn timestamp(&self) -> Option<u64> {
        self.imp().data.borrow().clone().and_then(|d| d.timestamp)
    }

    pub(super) fn quote_timestamp(&self) -> Option<u64> {
        self.data().and_then(|d| d.quote).and_then(|q| q.id)
    }

    pub fn set_quote(&self, msg: Message) {
        if let Some(mut data) = self.imp().data.borrow_mut().as_mut() {
            let sender = msg.property::<Contact>("sender").address();
            data.quote = Some(Quote {
                id: msg.timestamp(),
                author_e164: sender.as_ref().and_then(|a| a.e164()),
                author_uuid: sender.as_ref().and_then(|a| a.uuid).map(|u| u.to_string()),
                text: msg.property::<Option<String>>("body"),
                ..Default::default()
            });
        }
        self.imp().quote.replace(Some(msg));
    }

    pub(super) fn data(&self) -> Option<DataMessage> {
        self.imp().data.borrow().clone()
    }

    pub(super) fn reaction(&self) -> Option<Reaction> {
        self.imp().reaction.borrow().clone()
    }

    pub(super) fn react<S: AsRef<str>>(&self, reaction: S) {
        *self.imp().reactions.borrow_mut() += reaction.as_ref();
        self.notify("reactions");
    }

    pub async fn send_reaction<S: AsRef<str>>(
        &self,
        reaction: S,
    ) -> Result<(), crate::ApplicationError> {
        self.react(&reaction);
        let reaction_struct = Reaction {
            emoji: Some(reaction.as_ref().to_owned()),
            remove: Some(false),
            target_author_uuid: self
                .property::<Option<Contact>>("sender")
                .and_then(|s| s.address())
                .and_then(|a| a.uuid)
                .map(|u| u.to_string()),
            target_sent_timestamp: self.timestamp(),
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let message = DataMessage {
            timestamp: Some(timestamp),
            reaction: Some(reaction_struct),
            ..Default::default()
        };
        self.channel()
            .expect("Message to send reaction to to have channel")
            .send_internal_message(message, timestamp)
            .await
    }

    pub fn attachments(&self) -> Vec<Attachment> {
        self.imp().attachments.borrow().clone()
    }

    pub async fn add_attachment(
        &self,
        attachment: Attachment,
    ) -> Result<(), crate::ApplicationError> {
        log::trace!("Adding a attachment to a message");
        let manager = self.property::<Manager>("manager");
        let upload_data = attachment.as_upload_attachment().await;
        self.imp().attachments.borrow_mut().push(attachment);
        log::trace!("Uploading the attachment");
        let upload_attachments_result = manager.upload_attachments(vec![upload_data]).await?;

        let pointer = upload_attachments_result
            .first()
            .expect("At least one attachment pointer should be available")
            .as_ref()
            .expect("Failed to upload attachments");
        if let Some(data) = self.imp().data.borrow_mut().as_mut() {
            data.attachments.push(pointer.clone());
        }
        Ok(())
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::{
        glib::{
            once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecObject, ParamSpecString,
            ParamSpecUInt64, Value,
        },
        prelude::{ObjectExt, StaticType, ToValue},
    };
    use gtk::glib;
    use libsignal_service::content::Reaction;
    use presage::prelude::DataMessage;
    use std::cell::RefCell;

    use crate::backend::{Attachment, Channel, Contact, Manager};

    #[derive(Default)]
    pub struct Message {
        pub(super) channel: RefCell<Option<Channel>>,
        pub(super) sender: RefCell<Option<Contact>>,
        pub(super) data: RefCell<Option<DataMessage>>,

        pub(super) quote: RefCell<Option<super::Message>>,

        pub(super) reaction: RefCell<Option<Reaction>>,
        pub(super) reactions: RefCell<String>,

        pub(super) attachments: RefCell<Vec<Attachment>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "FlMessage";
        type Type = super::Message;
    }

    impl ObjectImpl for Message {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecObject::new(
                        "sender",
                        "sender",
                        "sender",
                        Contact::static_type(),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecUInt64::new(
                        "sent",
                        "sent",
                        "sent",
                        0,
                        u64::MAX,
                        0,
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new("body", "body", "body", None, ParamFlags::READABLE),
                    ParamSpecObject::new(
                        "quote",
                        "quote",
                        "quote",
                        super::Message::static_type(),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new(
                        "reactions",
                        "reactions",
                        "reactions",
                        Some(""),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new(
                        "textual-description",
                        "textual-description",
                        "textual-description",
                        Some(""),
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "sender" => self.sender.borrow().as_ref().to_value(),
                "body" => self
                    .data
                    .borrow()
                    .as_ref()
                    .and_then(|d| d.body.clone())
                    .to_value(),
                "reactions" => self.reactions.borrow().to_value(),
                "sent" => self
                    .data
                    .borrow()
                    .as_ref()
                    .and_then(|d| d.timestamp)
                    .unwrap_or(0)
                    .to_value(),
                "textual-description" => {
                    if let Some(body) = obj.property::<Option<String>>("body") {
                        body.to_value()
                    } else {
                        let attachments = self.attachments.borrow();

                        if attachments.iter().all(|a| a.is_image()) {
                            gettextrs::ngettext(
                                "Sent an image",
                                "Sent {} images",
                                attachments.len() as u32,
                            )
                            .replace("{}", &attachments.len().to_string())
                            .to_value()
                        } else if attachments.iter().all(|a| a.is_video()) {
                            gettextrs::ngettext(
                                "Sent an video",
                                "Sent {} videos",
                                attachments.len() as u32,
                            )
                            .replace("{}", &attachments.len().to_string())
                            .to_value()
                        } else {
                            gettextrs::ngettext(
                                "Sent a file",
                                "Sent {} files",
                                attachments.len() as u32,
                            )
                            .replace("{}", &attachments.len().to_string())
                            .to_value()
                        }
                    }
                }
                "quote" => self.quote.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Message` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
