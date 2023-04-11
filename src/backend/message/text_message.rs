use std::cell::RefCell;

use gdk::prelude::ObjectExt;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use gtk::{gdk, gio, glib};
use libsignal_service::proto::data_message::Delete;
use presage::prelude::content::Reaction;
use presage::prelude::{proto::data_message::Quote, *};

use crate::backend::timeline::{TimelineItem, TimelineItemExt};
use crate::backend::{Attachment, Channel, Contact};

use super::{DisplayMessage, Manager, Message, MessageExt};

gtk::glib::wrapper! {
    pub struct TextMessage(ObjectSubclass<imp::TextMessage>) @extends Message, DisplayMessage, TimelineItem;
}

impl TextMessage {
    pub fn textual_description(&self) -> String {
        self.property("textual-description")
    }

    pub fn body(&self) -> Option<String> {
        self.property("body")
    }

    pub fn reactions(&self) -> String {
        self.property("reactions")
    }

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
            .property("timestamp", &timestamp)
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
        log::info!("Just created TextMessage has {} references.", s.ref_count());
        s
    }

    pub(super) async fn init_data(&self, message: &DataMessage, manager: &Manager) {
        let obj = self.imp();
        self.set_internal_data(Some(message.clone()));
        let mut attachments = Vec::with_capacity(message.attachments.len());
        for pointer in &message.attachments {
            let att = Attachment::from_pointer(pointer, manager).await;
            attachments.push(att);
        }
        obj.attachments.swap(&RefCell::new(attachments));
    }

    pub fn quote(&self) -> Option<Quote> {
        self.internal_data().and_then(|d| d.quote)
    }

    pub fn set_quote(&self, msg: &TextMessage) {
        if let Some(mut data) = self.internal_data_mut().as_mut() {
            let sender = msg.sender().address();
            data.quote = Some(Quote {
                id: Some(msg.timestamp()),
                author_uuid: sender.as_ref().map(|a| a.uuid).map(|u| u.to_string()),
                text: msg.body(),
                ..Default::default()
            });
        }
        self.imp().quote.replace(Some(msg.clone()));
    }

    pub fn react<S: AsRef<str>>(&self, reaction: S) {
        *self.imp().reactions.borrow_mut() += reaction.as_ref();
        self.notify("reactions");
    }

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

    pub fn is_deleted(&self) -> bool {
        self.property("is-deleted")
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
                .sender()
                .address()
                .map(|a| a.uuid)
                .map(|u| u.to_string()),
            target_sent_timestamp: Some(self.timestamp()),
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
}

mod imp {
    use gdk::gdk_pixbuf::prelude::ToValue;
    use gdk::glib::ParamSpecBoolean;
    use gdk::prelude::ParamSpecBuilderExt;
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass, ObjectSubclassIsExt};
    use glib::{
        once_cell::sync::Lazy, subclass::prelude::ObjectSubclassExt, ParamSpec, ParamSpecObject,
        ParamSpecString, Value,
    };
    use gtk::{glib, prelude::Cast};
    use std::cell::RefCell;

    use crate::backend::{
        message::{display_message::DisplayMessageImpl, DisplayMessage, MessageExt, MessageImpl},
        Attachment,
    };
    use crate::backend::{
        timeline::{TimelineItem, TimelineItemImpl},
        Message,
    };

    #[derive(Default)]
    pub struct TextMessage {
        pub(super) quote: RefCell<Option<super::TextMessage>>,

        pub(super) reactions: RefCell<String>,

        pub(super) attachments: RefCell<Vec<Attachment>>,

        pub(super) is_deleted: RefCell<bool>,
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
    }

    impl DisplayMessageImpl for TextMessage {
        fn textual_description(&self, obj: &super::TextMessage) -> Option<String> {
            // let obj = obj
            //     .dynamic_cast_ref::<super::TextMessage>()
            //     .expect("Failed to cast `DisplayMessage` to `TextMessage`");
            if let Some(body) = obj.body() {
                body.lines().next().map(|s| s.to_owned())
            } else {
                let attachments = self.attachments.borrow();

                Some(if attachments.iter().all(|a| a.is_image()) {
                    gettextrs::ngettext("Sent an image", "Sent {} images", attachments.len() as u32)
                        .replace("{}", &attachments.len().to_string())
                } else if attachments.iter().all(|a| a.is_video()) {
                    gettextrs::ngettext("Sent an video", "Sent {} videos", attachments.len() as u32)
                        .replace("{}", &attachments.len().to_string())
                } else {
                    gettextrs::ngettext("Sent a file", "Sent {} files", attachments.len() as u32)
                        .replace("{}", &attachments.len().to_string())
                })
            }
        }
    }
    impl MessageImpl for TextMessage {}

    impl ObjectImpl for TextMessage {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("body").read_only().build(),
                    ParamSpecObject::builder::<super::TextMessage>("quote")
                        .read_only()
                        .build(),
                    ParamSpecString::builder("reactions")
                        .default_value(Some(""))
                        .read_only()
                        .build(),
                    ParamSpecBoolean::builder("is-deleted").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            let instance = self.obj();
            match pspec.name() {
                "body" => instance.internal_data().and_then(|d| d.body).to_value(),
                "reactions" => self.reactions.borrow().to_value(),
                "quote" => self.quote.borrow().to_value(),
                "is-deleted" => self.is_deleted.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "is-deleted" => {
                    let obj = value
                        .get::<bool>()
                        .expect("Property `is-deleted` of `TextMessage` has to be of type `bool`");

                    self.is_deleted.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
