mod call_message;
mod display_message;
mod reaction_message;
mod text_message;

pub use call_message::CallMessage;
pub use display_message::DisplayMessage;
pub use reaction_message::ReactionMessage;
pub use text_message::TextMessage;

use glib::{
    subclass::{
        prelude::ObjectImpl,
        types::{IsSubclassable, ObjectSubclassIsExt},
    },
    Object,
};
use gtk::{glib, prelude::*};
use presage::prelude::{
    content::sync_message::Sent, Content, ContentBody, DataMessage, ServiceAddress, SyncMessage,
};
use std::cell::{RefCell, RefMut};

use crate::backend::Channel;

use super::{Contact, Manager};

glib::wrapper! {
    pub struct Message(ObjectSubclass<imp::Message>);
}

impl Message {
    pub(super) async fn from_content(content: Content, manager: &Manager) -> Option<Self> {
        log::trace!("Trying to build a message from content");
        let metadata = &content.metadata;
        let contact = Contact::from_service_address(&metadata.sender, manager);

        let body = &content.body;
        let timestamp = metadata.timestamp;

        match body {
            ContentBody::DataMessage(message) if message.reaction.is_none() => {
                let channel =
                    Channel::from_contact_or_group(contact.clone(), &message.group_v2, manager)
                        .await;
                contact.set_channel(Some(&channel));
                if message.body.is_none() && message.attachments.is_empty() {
                    return None;
                }
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("sent", &timestamp)
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
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
            }) if message.reaction.is_none() => {
                if message.body.is_none() && message.attachments.is_empty() {
                    return None;
                }
                let destination_contact = if e164.is_some() || uuid.is_some() {
                    let destination_address = ServiceAddress {
                        // TODO: Change reaction message to UUID
                        uuid: uuid
                            .clone()
                            .unwrap_or_default()
                            .parse()
                            .expect("Failed to parse UUID"),
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
                contact.set_channel(Some(&channel));
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("sent", &timestamp)
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
            }
            ContentBody::DataMessage(message) if message.reaction.is_some() => {
                let channel =
                    Channel::from_contact_or_group(contact.clone(), &message.group_v2, manager)
                        .await;
                contact.set_channel(Some(&channel));
                Some(
                    ReactionMessage::from_reaction(
                        &contact,
                        &channel,
                        timestamp,
                        manager,
                        message.reaction.as_ref().unwrap().clone(),
                    )
                    .upcast(),
                )
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
            }) if message.reaction.is_some() => {
                let destination_contact = if e164.is_some() || uuid.is_some() {
                    let destination_address = ServiceAddress {
                        // TODO: Change reaction message to UUID
                        uuid: uuid
                            .clone()
                            .unwrap_or_default()
                            .parse()
                            .expect("Failed to parse UUID"),
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
                contact.set_channel(Some(&channel));
                Some(
                    ReactionMessage::from_reaction(
                        &contact,
                        &channel,
                        timestamp,
                        manager,
                        message.reaction.as_ref().unwrap().clone(),
                    )
                    .upcast(),
                )
            }
            ContentBody::SynchronizeMessage(SyncMessage { read: read_arr, .. })
                if !read_arr.is_empty() =>
            {
                log::trace!("Got currently unhandled read-message");
                None
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                viewed: viewed_arr, ..
            }) if !viewed_arr.is_empty() => {
                log::trace!("Got currently unhandled viewed-message");
                None
            }
            ContentBody::CallMessage(c) => {
                // TODO: Group calls?
                let channel = Channel::from_contact_or_group(contact.clone(), &None, manager).await;
                contact.set_channel(Some(&channel));
                CallMessage::from_call(&contact, &channel, timestamp, manager, c.clone())
                    .map(|c| c.upcast())
            }
            ContentBody::TypingMessage(_) => {
                log::trace!("Got currently unhandled call-message");
                None
            }
            ContentBody::ReceiptMessage(_) => {
                log::trace!("Got currently unhandled receipt-message");
                None
            }
            _ => {
                log::info!("Do not know what to do with the message: {:?}", content);
                None
            }
        }
    }

    fn set_internal_data(&self, data: Option<DataMessage>) {
        self.imp().data.swap(&RefCell::new(data));
    }

    fn internal_data(&self) -> Option<DataMessage> {
        self.imp().data.borrow().clone()
    }

    fn internal_data_mut(&self) -> RefMut<'_, Option<DataMessage>> {
        self.imp().data.borrow_mut()
    }
}

pub trait MessageExt: std::marker::Sized + glib::ObjectExt {
    fn set_internal_data(&self, data: Option<DataMessage>) {
        self.dynamic_cast_ref::<Message>()
            .expect("`MessageExt` to dynamic cast to `Message`")
            .set_internal_data(data)
    }

    fn internal_data(&self) -> Option<DataMessage> {
        self.dynamic_cast_ref::<Message>()
            .expect("`MessageExt` to dynamic cast to `Message`")
            .internal_data()
    }

    fn internal_data_mut(&self) -> RefMut<'_, Option<DataMessage>> {
        self.dynamic_cast_ref::<Message>()
            .expect("`MessageExt` to dynamic cast to `Message`")
            .internal_data_mut()
    }

    fn sender(&self) -> Contact {
        self.property("sender")
    }

    fn channel(&self) -> Channel {
        self.property("channel")
    }

    fn sent(&self) -> u64 {
        self.property("sent")
    }

    fn manager(&self) -> Manager {
        self.property("manager")
    }
}

impl<O: IsA<Message>> MessageExt for O {}

pub trait MessageImpl: ObjectImpl {}
unsafe impl<T: MessageImpl> IsSubclassable<T> for Message {}

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::{
        once_cell::sync::Lazy, subclass::types::ObjectSubclass, ParamSpec, ParamSpecObject,
        ParamSpecUInt64, Value,
    };

    use crate::backend::Manager;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Message {
        sender: RefCell<Option<Contact>>,
        channel: RefCell<Option<Channel>>,
        sent: Cell<u64>,

        pub(super) data: RefCell<Option<DataMessage>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "FlMessage";
        type Type = super::Message;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Message {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Contact>("sender")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Channel>("channel")
                        .construct_only()
                        .build(),
                    ParamSpecUInt64::builder("sent").construct_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "sender" => self.sender.borrow().as_ref().to_value(),
                "channel" => self.channel.borrow().as_ref().to_value(),
                "sent" => self.sent.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Message` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                "sender" => {
                    let obj = value
                        .get::<Contact>()
                        .expect("Property `sender` of `Message` has to be of type `Contact`");

                    self.sender.replace(Some(obj));
                }
                "channel" => {
                    let obj = value
                        .get::<Channel>()
                        .expect("Property `channel` of `Message` has to be of type `Channel`");

                    self.channel.replace(Some(obj));
                }
                "sent" => {
                    let obj = value
                        .get::<u64>()
                        .expect("Property `sent` of `Message` has to be of type `u64`");

                    self.sent.set(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
