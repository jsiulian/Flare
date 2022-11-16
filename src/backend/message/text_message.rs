use std::cell::RefCell;

use gdk::prelude::ObjectExt;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use presage::prelude::content::Reaction;
use presage::prelude::{proto::data_message::Quote, *};

use crate::backend::{Attachment, Channel, Contact};

use super::{DisplayMessage, Manager, Message, MessageExt};

gtk::glib::wrapper! {
    pub struct TextMessage(ObjectSubclass<imp::TextMessage>) @extends Message, DisplayMessage;
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
        let s: Self = Object::new(&[
            ("manager", manager),
            ("channel", &channel),
            ("sender", &sender),
            ("sent", &timestamp),
        ])
        .expect("Failed to create `Message`");

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
                id: Some(msg.sent()),
                author_e164: sender.as_ref().and_then(|a| a.e164()),
                author_uuid: sender.as_ref().and_then(|a| a.uuid).map(|u| u.to_string()),
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
                .and_then(|a| a.uuid)
                .map(|u| u.to_string()),
            target_sent_timestamp: Some(self.sent()),
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
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::{
        glib::{
            once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecObject, ParamSpecString, Value,
        },
        prelude::{StaticType, ToValue},
    };
    use gtk::glib;
    use std::cell::RefCell;

    use crate::backend::{
        message::{display_message::DisplayMessageImpl, DisplayMessage, MessageExt, MessageImpl},
        Attachment,
    };

    #[derive(Default)]
    pub struct TextMessage {
        pub(super) quote: RefCell<Option<super::TextMessage>>,

        pub(super) reactions: RefCell<String>,

        pub(super) attachments: RefCell<Vec<Attachment>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextMessage {
        const NAME: &'static str = "FlTextMessage";
        type Type = super::TextMessage;
        type ParentType = DisplayMessage;
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
                    ParamSpecString::new("body", "body", "body", None, ParamFlags::READABLE),
                    ParamSpecObject::new(
                        "quote",
                        "quote",
                        "quote",
                        super::TextMessage::static_type(),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new(
                        "reactions",
                        "reactions",
                        "reactions",
                        Some(""),
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "body" => obj.internal_data().and_then(|d| d.body).to_value(),
                "reactions" => self.reactions.borrow().to_value(),
                "quote" => self.quote.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, _value: &Value, _pspec: &ParamSpec) {
            unimplemented!();
        }
    }
}
