use crate::prelude::*;

use libsignal_service::content::Reaction;
use libsignal_service::proto::body_range::AssociatedValue;
use libsignal_service::proto::data_message::Delete;
use libsignal_service::proto::DataMessage;
use pango::{AttrColor, AttrList};

use crate::backend::timeline::{TimelineItem, TimelineItemExt};
use crate::backend::{Attachment, Channel, Contact};

use super::{DisplayMessage, Manager, Message, MessageExt, ReactionMessage};

gtk::glib::wrapper! {
    /// A message which contains either text, attachments or both.
    pub struct TextMessage(ObjectSubclass<imp::TextMessage>) @extends Message, DisplayMessage, TimelineItem;
}

const MENTION_CHAR: char = '@';
const MENTION_COLOR: (u16, u16, u16) = (0, 0, u16::MAX);

impl TextMessage {
    pub fn from_text_channel_sender<S: AsRef<str>>(
        text: S,
        channel: Channel,
        sender: Contact,
        manager: &Manager,
    ) -> Self {
        Self::from_text_channel_sender_timestamp(
            text,
            channel,
            sender,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis() as u64,
            manager,
        )
    }

    #[cfg(feature = "screenshot")]
    pub fn pub_from_text_channel_sender_timestamp<S: AsRef<str>>(
        text: S,
        channel: Channel,
        sender: Contact,
        timestamp: u64,
        manager: &Manager,
    ) -> Self {
        Self::from_text_channel_sender_timestamp(text, channel, sender, timestamp, manager)
    }

    fn from_text_channel_sender_timestamp<S: AsRef<str>>(
        text: S,
        channel: Channel,
        sender: Contact,
        timestamp: u64,
        manager: &Manager,
    ) -> Self {
        log::trace!("Trying to build a message from text");
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .property("channel", &channel)
            .property("sender", &sender)
            .property("timestamp", timestamp)
            .build();

        let text_owned = text.as_ref().to_owned();
        let body = if text_owned.is_empty() {
            None
        } else {
            Some(text_owned)
        };

        let message = DataMessage {
            body,
            timestamp: Some(timestamp),
            ..Default::default()
        };

        s.set_internal_data(Some(message));
        s.prepare_format_body();
        s
    }

    /// Asynchronously complete constructing the message.
    pub(super) async fn init_data(&self, message: &DataMessage, manager: &Manager) {
        let obj = self.imp();
        self.set_internal_data(Some(message.clone()));
        self.prepare_format_body();

        // Load attachments in parallel.
        let attachment_futures = message
            .attachments
            .iter()
            .map(|pointer| async move { Attachment::from_pointer(pointer, manager).await });
        let attachments = futures::future::join_all(attachment_futures).await;
        obj.attachments.swap(&RefCell::new(attachments));
    }

    /// Apply a reaction message.
    pub fn react(&self, reaction: &ReactionMessage) {
        self.react_sender_reaction(reaction.sender().uuid(), reaction.reaction());
    }

    /// Apply a reaction from a sender.
    fn react_sender_reaction(&self, sender: Uuid, reaction: Reaction) {
        if reaction.remove.unwrap_or_default() {
            self.imp().reactions.borrow_mut().remove(&sender);
        } else {
            let _ = self.imp().reactions.borrow_mut().insert(sender, reaction);
        }
        self.notify("reactions");
    }

    pub fn quote_timestamp(&self) -> Option<u64> {
        self.internal_data()
            .and_then(|d| d.quote)
            .and_then(|q| q.id)
    }

    /// Sends a deletion request for a message and marks the message as deleted.
    pub async fn delete(&self) -> Result<(), crate::ApplicationError> {
        crate::trace!("Delete a message with timestamp: {}", self.timestamp());
        let delete = Delete {
            target_sent_timestamp: Some(self.timestamp()),
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let message = DataMessage {
            timestamp: Some(timestamp),
            delete: Some(delete),
            ..Default::default()
        };
        self.channel()
            .send_internal_message(message, timestamp)
            .await?;

        self.mark_as_deleted();
        self.channel().notify("last-message");

        Ok(())
    }

    pub fn mark_as_deleted(&self) {
        // TODO: Does not delete message from memory.
        self.set_property("is-deleted", true);
    }

    /// Send a reaction for a message and apply it.
    pub async fn send_reaction<S: AsRef<str>>(
        &self,
        reaction: S,
    ) -> Result<(), crate::ApplicationError> {
        // Instead of adding a reaction, remove it if there is already a reaction.
        let has_self_reaction = self
            .imp()
            .reactions
            .borrow()
            .contains_key(&self.manager().uuid());

        let reaction_struct = Reaction {
            emoji: Some(reaction.as_ref().to_owned()),
            remove: Some(has_self_reaction),
            target_author_aci: self
                .sender()
                .address()
                .map(|a| a.uuid)
                .map(|u| u.to_string()),
            target_sent_timestamp: Some(self.timestamp()),
        };
        self.react_sender_reaction(self.manager().uuid(), reaction_struct.clone());

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
            .send_internal_message(message, timestamp)
            .await
    }

    pub fn attachments(&self) -> Vec<Attachment> {
        self.imp().attachments.borrow().clone()
    }

    /// Adds the attachment to the message by uploading it and adding the resulting attachment pointer to the internal data.
    pub async fn add_attachment(
        &self,
        attachment: Attachment,
    ) -> Result<(), crate::ApplicationError> {
        log::trace!("Adding a attachment to a message");
        let manager = self.manager();
        let upload_data = attachment.as_upload_attachment().await;
        self.imp().attachments.borrow_mut().push(attachment);
        log::trace!("Uploading the attachment");
        let upload_attachments_result = manager.upload_attachments(vec![upload_data]).await?;

        let pointer = upload_attachments_result
            .first()
            .expect("At least one attachment pointer should be available")
            .as_ref()
            .expect("Failed to upload attachments");
        if let Some(data) = self.internal_data_mut().as_mut() {
            data.attachments.push(pointer.clone());
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.body().is_none() && self.attachments().is_empty()
    }

    fn prepare_format_body(&self) {
        let (body, attrs) = self.format_body();
        *self.imp().formatted_body.borrow_mut() = body;
        *self.imp().message_attributes.borrow_mut() = attrs;
    }

    /// Formats the message body based on its ranges, e.g. to insert mention names.
    ///
    /// Returns the resulting strings and an [AttrList] that can be used in labels to highlight areas.
    /// Be carefull when editing this function and note that Signal uses UTF-16 byte offsets, while Rust uses UTF-8 byte offsets.
    fn format_body(&self) -> (Option<String>, AttrList) {
        let Some(body) = self.internal_data().and_then(|m| m.body) else {
            return (None, AttrList::new());
        };
        let mut ranges = self
            .internal_data()
            .map(|m| m.body_ranges)
            .unwrap_or_default();

        if ranges.is_empty() {
            return (Some(body), AttrList::new());
        }

        let channel = self.channel();

        // Sort by growing start index
        ranges.sort_unstable_by_key(|r| r.start());

        let attrs = AttrList::new();

        // Signal (Java) uses UTF-16 body and therefore also UTF-16 offsets, while Flare (Rust) uses UTF-8. Need to convert.
        let body_utf16: Vec<u16> = body.encode_utf16().collect();

        let mut result_utf8 = String::new();
        let mut index_utf16 = 0;
        let mut index_utf8 = 0;
        for r in ranges {
            let start = r.start() as usize;
            let end = start + r.length() as usize;
            let Some(AssociatedValue::MentionAci(u)) = r.associated_value else {
                continue;
            };
            let Ok(uuid) = u.parse() else { continue };
            let name = format!(
                "{}{}",
                MENTION_CHAR,
                channel.participant_by_uuid(uuid).title()
            );
            let to_add_body = String::from_utf16_lossy(&body_utf16[index_utf16..start]);
            result_utf8.push_str(&to_add_body);
            result_utf8.push_str(&name);
            index_utf16 = end;

            let index_start_highlight = index_utf8 + to_add_body.len();
            index_utf8 += to_add_body.len() + name.len();
            let index_end_highlight = index_utf8;

            let mut highlight =
                AttrColor::new_foreground(MENTION_COLOR.0, MENTION_COLOR.1, MENTION_COLOR.2);
            highlight.set_start_index(index_start_highlight as u32);
            highlight.set_end_index(index_end_highlight as u32);
            attrs.insert(highlight);
        }

        if index_utf16 < body_utf16.len() {
            result_utf8.push_str(&String::from_utf16_lossy(&body_utf16[index_utf16..]))
        }

        (Some(result_utf8), attrs)
    }
}

mod imp {
    use crate::prelude::*;
    use libsignal_service::content::Reaction;
    use pango::AttrList;
    use presage::proto::data_message::Quote;
    use std::collections::HashMap;

    use crate::backend::message::MessageExt;
    use crate::backend::timeline::TimelineItemExt;
    use crate::backend::{
        message::{display_message::DisplayMessageImpl, DisplayMessage, MessageImpl},
        Attachment,
    };
    use crate::backend::{
        timeline::{TimelineItem, TimelineItemImpl},
        Message,
    };

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::TextMessage)]
    pub struct TextMessage {
        #[property(get, set = Self::set_quote)]
        pub(super) quote: RefCell<Option<super::TextMessage>>,

        #[property(get = Self::reactions, type = String, default = Some(""))]
        pub(super) reactions: RefCell<HashMap<Uuid, Reaction>>,

        pub(super) attachments: RefCell<Vec<Attachment>>,

        #[property(name = "body", get)]
        pub(super) formatted_body: RefCell<Option<String>>,
        #[property(get)]
        pub(super) message_attributes: RefCell<AttrList>,
        #[property(get, set)]
        pub(super) is_deleted: RefCell<bool>,
    }

    impl TextMessage {
        fn reactions(&self) -> String {
            let mut reaction_emojis: Vec<String> = Vec::new();
            let mut reaction_counts: Vec<i32> = Vec::new();

            // This counts the number of each emoji
            self.reactions.borrow().values().for_each(|r| {
                match reaction_emojis.iter().position(|e| e == r.emoji()) {
                    Some(index) => reaction_counts[index] += 1,
                    None => {
                        reaction_emojis.push(r.emoji().to_string());
                        reaction_counts.push(1);
                    }
                }
            });

            // And this creates a string with the emojis and their counts
            reaction_emojis
                .iter()
                .zip(reaction_counts.iter())
                .map(|(emoji, &count)| {
                    if count > 1 {
                        format!("{}{}", emoji, count)
                    } else {
                        emoji.clone()
                    }
                })
                .collect::<Vec<String>>()
                .join(" ")
        }

        fn set_quote(&self, msg: &super::TextMessage) {
            if let Some(data) = self.obj().internal_data_mut().as_mut() {
                let sender = msg.sender().address();
                data.quote = Some(Quote {
                    id: Some(msg.timestamp()),
                    author_aci: sender.as_ref().map(|a| a.uuid).map(|u| u.to_string()),
                    text: msg.body(),
                    ..Default::default()
                });
            }
            self.quote.replace(Some(msg.clone()));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextMessage {
        const NAME: &'static str = "FlTextMessage";
        type Type = super::TextMessage;
        type ParentType = DisplayMessage;
    }

    impl TimelineItemImpl for TextMessage {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_header(upcast, previous);
        }
        fn update_show_timestamp(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_timestamp(upcast, previous);
        }
    }

    impl DisplayMessageImpl for TextMessage {
        fn textual_description(&self, obj: &super::TextMessage) -> Option<String> {
            if let Some(body) = obj.body() {
                body.lines().next().map(|s| s.to_owned())
            } else {
                let attachments = self.attachments.borrow();

                let formatter = if attachments.iter().all(|a| a.is_image()) {
                    gettextrs::ngettext("Sent an image", "Sent {} images", attachments.len() as u32)
                } else if attachments.iter().all(|a| a.is_video()) {
                    gettextrs::ngettext("Sent an video", "Sent {} videos", attachments.len() as u32)
                } else if attachments.iter().all(|a| a.is_audio()) {
                    gettextrs::ngettext(
                        "Sent a voice message",
                        "Sent {} voice messages",
                        attachments.len() as u32,
                    )
                } else {
                    gettextrs::ngettext("Sent a file", "Sent {} files", attachments.len() as u32)
                };

                Some(formatter.replace("{}", &attachments.len().to_string()))
            }
        }
    }
    impl MessageImpl for TextMessage {}

    #[glib::derived_properties]
    impl ObjectImpl for TextMessage {}
}
