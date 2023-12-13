mod call_message;
mod deletion_message;
mod display_message;
mod reaction_message;
mod text_message;

pub use call_message::CallMessage;
pub use deletion_message::DeletionMessage;
pub use display_message::{DisplayMessage, DisplayMessageExt};
use libsignal_service::{
    content::ContentBody,
    prelude::Content,
    proto::{sync_message::Sent, DataMessage, SyncMessage},
};
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
use std::cell::{RefCell, RefMut};

use crate::backend::Channel;

use super::{
    timeline::{TimelineItem, TimelineItemImpl},
    Contact, Manager,
};

glib::wrapper! {
    pub struct Message(ObjectSubclass<imp::Message>)
    @extends TimelineItem;
}

impl Message {
    pub(super) async fn from_content(content: Content, manager: &Manager) -> Option<Self> {
        log::trace!("Trying to build a message from content");
        let metadata = &content.metadata;

        let body = &content.body;
        let timestamp = metadata.timestamp;

        match body {
            ContentBody::DataMessage(message)
                if message.reaction.is_none() && message.delete.is_none() =>
            {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender.uuid, &message.group_v2)
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }

                if message.body.is_none() && message.attachments.is_empty() {
                    return None;
                }
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("timestamp", timestamp)
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(Sent {
                        destination_service_id: uuid,
                        message: Some(message),
                        ..
                    }),
                ..
            }) if message.reaction.is_none() && message.delete.is_none() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        uuid.as_ref()
                            .map(|u| u.parse().expect("Failed to parse UUID"))
                            .unwrap_or(metadata.sender.uuid),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                if message.body.is_none() && message.attachments.is_empty() {
                    return None;
                }
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("timestamp", timestamp)
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
            }
            ContentBody::DataMessage(message) if message.reaction.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender.uuid, &message.group_v2)
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
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
                        destination_service_id: uuid,
                        message: Some(message),
                        ..
                    }),
                ..
            }) if message.reaction.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        uuid.as_ref()
                            .map(|u| u.parse().expect("Failed to parse UUID"))
                            .unwrap_or(metadata.sender.uuid),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
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
            ContentBody::DataMessage(message) if message.delete.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender.uuid, &message.group_v2)
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                log::trace!("Got a deletion message");
                Some(
                    DeletionMessage::from_delete(
                        &contact,
                        &channel,
                        timestamp,
                        manager,
                        message.delete.as_ref().unwrap().clone(),
                    )
                    .upcast(),
                )
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(Sent {
                        destination_service_id: uuid,
                        message: Some(message),
                        ..
                    }),
                ..
            }) if message.delete.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        uuid.as_ref()
                            .map(|u| u.parse().expect("Failed to parse UUID"))
                            .unwrap_or(metadata.sender.uuid),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                log::trace!("Got a deletion message");
                Some(
                    DeletionMessage::from_delete(
                        &contact,
                        &channel,
                        timestamp,
                        manager,
                        message.delete.as_ref().unwrap().clone(),
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
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender.uuid, &None)
                    .await;
                let contact = channel.participant_by_uuid(metadata.sender.uuid);
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                CallMessage::from_call(&contact, &channel, timestamp, manager, c.clone())
                    .map(|c| c.upcast())
            }
            ContentBody::TypingMessage(_) => {
                log::trace!("Got currently unhandled typing-message");
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

    fn manager(&self) -> Manager {
        self.property("manager")
    }
}

impl<O: IsA<Message>> MessageExt for O {}

pub trait MessageImpl: ObjectImpl {}

unsafe impl<T> IsSubclassable<T> for Message
where
    T: MessageImpl + TimelineItemImpl,
    T::Type: IsA<Message> + IsA<TimelineItem>,
{
}

mod imp {
    use std::cell::RefCell;

    use glib::{
        once_cell::sync::Lazy, subclass::types::ObjectSubclass, ParamSpec, ParamSpecObject, Value,
    };

    use crate::backend::{timeline::TimelineItemExt, Manager};

    use super::*;

    #[derive(Debug, Default)]
    pub struct Message {
        sender: RefCell<Option<Contact>>,
        channel: RefCell<Option<Channel>>,

        pub(super) data: RefCell<Option<DataMessage>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "FlMessage";
        type Type = super::Message;
        type ParentType = TimelineItem;
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
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "sender" => self.sender.borrow().as_ref().to_value(),
                "channel" => self.channel.borrow().as_ref().to_value(),
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
                _ => unimplemented!(),
            }
        }
    }

    // At least 4 minutes need to pass such that for two messages from the same sender, the second one will
    // also show avatar and sender title.
    const MESSAGE_SENT_SHOW_NAME_DURATION: u64 = 4 * 60 * 1000;
    // At least 1 minute need to pass such that for two messages from the same sender, the second one will
    // also show the timestamp.
    const MESSAGE_SENT_SHOW_TIMESTAMP_DURATION: u64 = 60 * 1000;

    impl TimelineItemImpl for Message {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            if let Some(msg) = previous.and_then(|p| p.downcast_ref::<super::Message>()) {
                obj.set_show_header(
                    obj.sender().uuid() != msg.sender().uuid()
                        || obj.timestamp() >= msg.timestamp() + MESSAGE_SENT_SHOW_NAME_DURATION,
                );
            } else {
                obj.set_show_header(true);
            }
        }
        fn update_show_timestamp(&self, obj: &Self::Type, next: Option<&TimelineItem>) {
            if let Some(msg) = next.and_then(|p| p.downcast_ref::<super::Message>()) {
                obj.set_show_timestamp(
                    obj.sender().uuid() != msg.sender().uuid()
                        || msg.timestamp()
                            >= obj.timestamp() + MESSAGE_SENT_SHOW_TIMESTAMP_DURATION,
                );
            } else {
                obj.set_show_timestamp(true);
            }
        }
    }
}
