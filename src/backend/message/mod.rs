mod call_message;
mod deletion_message;
mod display_message;
mod reaction_message;
mod text_message;

pub use call_message::{CallMessage, CallMessageType};
pub use deletion_message::DeletionMessage;
pub use display_message::{DisplayMessage, DisplayMessageExt};
pub use reaction_message::ReactionMessage;
pub use text_message::TextMessage;

use super::{
    Contact, Manager,
    timeline::{TimelineItem, TimelineItemImpl},
};
use crate::backend::{Channel, channel::TypingNotification};
use crate::prelude::*;

use glib::{
    Object,
    subclass::{
        prelude::ObjectImpl,
        types::{IsSubclassable, ObjectSubclassIsExt},
    },
};
use std::cell::RefMut;

use libsignal_service::proto::typing_message::Action;
use libsignal_service::{
    content::ContentBody,
    proto::{DataMessage, SyncMessage, sync_message::Sent},
};

/// At least 4 minutes need to pass such that for two messages from the same sender, the second one will
/// also show avatar and sender title.
const MESSAGE_SENT_SHOW_NAME_DURATION: u64 = 4 * 60 * 1000;
/// At least 1 minute need to pass such that for two messages from the same sender, the second one will
/// also show the timestamp.
const MESSAGE_SENT_SHOW_TIMESTAMP_DURATION: u64 = 60 * 1000;

glib::wrapper! {
    /// A generic message. Note that not every message needs to be displayed to the user, e.g. deletion messages.
    ///
    /// In general, a [Message] is just something holding [Content].
    pub struct Message(ObjectSubclass<imp::Message>)
    @extends TimelineItem;
}

impl Message {
    /// Big function to create all different message types from the Content.
    pub(super) async fn from_content(content: Content, manager: &Manager) -> Option<Self> {
        log::trace!("Trying to build a message from content");
        let metadata = &content.metadata;

        let body = &content.body;
        let timestamp = metadata.timestamp;

        match body {
            // A normal text message.
            ContentBody::DataMessage(message)
                if message.reaction.is_none() && message.delete.is_none() =>
            {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender, &message.group_v2)
                    .await;

                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;

                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }

                if message.body.is_none()
                    && message.attachments.is_empty()
                    && message.sticker.is_none()
                {
                    return None;
                }
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("timestamp", timestamp)
                    .property("read", channel.property::<bool>("is-active"))
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
            }
            // A normal text message, sent from another device.
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(
                        sent @ Sent {
                            message: Some(message),
                            ..
                        },
                    ),
                ..
            }) if message.reaction.is_none() && message.delete.is_none() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        sent.parse_destination_service_id()
                            .unwrap_or(metadata.sender),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                if message.body.is_none()
                    && message.attachments.is_empty()
                    && message.sticker.is_none()
                {
                    return None;
                }
                let s: TextMessage = Object::builder::<TextMessage>()
                    .property("manager", manager)
                    .property("sender", &contact)
                    .property("channel", &channel)
                    .property("timestamp", timestamp)
                    .property("read", channel.property::<bool>("is-active"))
                    .build();
                s.init_data(message, manager).await;
                Some(s.upcast())
            }
            // A reaction message.
            ContentBody::DataMessage(message) if message.reaction.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender, &message.group_v2)
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
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
            // A reaction message, sent from another device.
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(
                        sent @ Sent {
                            message: Some(message),
                            ..
                        },
                    ),
                ..
            }) if message.reaction.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        sent.parse_destination_service_id()
                            .unwrap_or(metadata.sender),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
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
            // A deletion message.
            ContentBody::DataMessage(message) if message.delete.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender, &message.group_v2)
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
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
                        *message.delete.as_ref().unwrap(),
                    )
                    .upcast(),
                )
            }
            // A deletion message, sent from another device.
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(
                        sent @ Sent {
                            message: Some(message),
                            ..
                        },
                    ),
                ..
            }) if message.delete.is_some() => {
                let channel = manager
                    .channel_from_uuid_or_group(
                        sent.parse_destination_service_id()
                            .unwrap_or(metadata.sender),
                        &message.group_v2,
                    )
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
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
                        *message.delete.as_ref().unwrap(),
                    )
                    .upcast(),
                )
            }
            // Call message.
            ContentBody::CallMessage(c) => {
                // TODO: Group calls?
                let channel = manager
                    .channel_from_uuid_or_group(metadata.sender, &None)
                    .await;
                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                    return None;
                }
                CallMessage::from_call(&contact, &channel, timestamp, manager, c.clone())
                    .map(|c| c.upcast())
            }
            // Typing messages.
            // Note that they are currently only implemented for contacts, this requires upstream updates to fix.
            ContentBody::TypingMessage(t) => {
                let channel = if let Some(id) = &t.group_id {
                    manager.channel_from_group_id(id)
                } else {
                    Some(
                        manager
                            .channel_from_uuid_or_group(metadata.sender, &None)
                            .await,
                    )
                };

                let Some(channel) = channel else {
                    log::warn!("Got typing message from channel that cannot be found; aborting");
                    return None;
                };

                let contact = channel
                    .participant_by_uuid(metadata.sender.raw_uuid())
                    .await;
                if contact.is_blocked() {
                    log::debug!("Got message from a blocked contact. Ignoring");
                } else {
                    match t.action() {
                        Action::Started => channel.add_user_typing(TypingNotification {
                            sender: contact,
                            timestamp: t.timestamp(),
                        }),
                        Action::Stopped => channel.remove_user_typing(contact),
                    };
                }
                None
            }

            // Currently unhandled messages
            ContentBody::SynchronizeMessage(SyncMessage { read: read_arr, .. })
                if !read_arr.is_empty() =>
            {
                log::trace!("Got currently unhandled read-message");
                for r in read_arr {
                    let Some(timestamp) = r.timestamp else {
                        continue;
                    };
                    let Some(sender_uuid) = r.parse_sender_aci().map(Uuid::from) else {
                        continue;
                    };
                    // Note: Matches the implementation of Message:uid.
                    let notification_id = format!("{timestamp:x}{sender_uuid:x}");
                    manager.withdraw_notification(&notification_id);
                }

                None
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                viewed: viewed_arr, ..
            }) if !viewed_arr.is_empty() => {
                log::trace!("Got currently unhandled viewed-message");
                None
            }
            ContentBody::ReceiptMessage(_) => {
                log::trace!("Got currently unhandled receipt-message");
                None
            }
            _ => {
                log::debug!("Do not know what to do with the message: {:?}", content);
                None
            }
        }
    }

    // A unique ID of the message, built from the timestamp and sender.
    pub fn uid(&self) -> Option<String> {
        let data = self.imp().data.borrow().clone()?;
        let timestamp = data.timestamp?;
        let sender_uuid = self.sender().uuid();
        Some(format!("{timestamp:x}{sender_uuid:x}"))
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

    fn mark_as_read(&self) -> bool {
        let this = self.imp();
        let read = *this.read.borrow();
        if !read {
            this.read.replace(true);
            self.notify("read");
            return true;
        }
        false
    }
}

pub trait MessageExt: std::marker::Sized + glib::prelude::ObjectExt {
    fn uid(&self) -> Option<String> {
        self.dynamic_cast_ref::<Message>()
            .expect("`MessageExt` to dynamic cast to `Message`")
            .uid()
    }

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

    fn mark_as_read(&self) -> bool {
        self.dynamic_cast_ref::<Message>()
            .expect("`MessageExt` to dynamic cast to `Message`")
            .mark_as_read()
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
    use crate::backend::{Manager, timeline::TimelineItemExt};

    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::Message)]
    pub struct Message {
        #[property(get, set, construct_only, type = Contact)]
        sender: RefCell<Option<Contact>>,
        #[property(get, set, construct_only, type = Channel)]
        channel: RefCell<Option<Channel>>,
        #[property(get, set, construct_only)]
        pub(super) read: RefCell<bool>,
        #[property(get, set)]
        pub(super) pending: RefCell<bool>,
        #[property(get, set)]
        pub(super) error: RefCell<bool>,

        pub(super) data: RefCell<Option<DataMessage>>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "FlMessage";
        type Type = super::Message;
        type ParentType = TimelineItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Message {}

    impl TimelineItemImpl for Message {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            if let Some(msg) = previous.and_then(|p| p.downcast_ref::<super::Message>()) {
                // Show header if:
                // - The sender of the previous message is different to the sender of this message, or
                // - Some time has passed between the two messages.
                // Furthermore, never show headers for messages sent by self.
                obj.set_show_header(
                    (obj.sender().uuid() != msg.sender().uuid()
                        || obj.timestamp() >= msg.timestamp() + MESSAGE_SENT_SHOW_NAME_DURATION)
                        && obj.sender().uuid() != obj.manager().uuid(),
                );
            } else {
                obj.set_show_header(true);
            }
        }

        fn update_show_timestamp(&self, obj: &Self::Type, next: Option<&TimelineItem>) {
            if let Some(msg) = next.and_then(|p| p.downcast_ref::<super::Message>()) {
                // Show timestamp if:
                // - The sender of the previous message is different to the sender of this message, or
                // - Some time has passed between the two messages.
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
