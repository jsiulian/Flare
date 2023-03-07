use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gdk::prelude::ObjectExt;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{Cast, Object};
use gtk::{gdk, gio, glib};
use libsignal_service::groups_v2::Group;
use presage::{
    prelude::{DataMessage, GroupContextV2, Uuid},
    Thread,
};

use crate::backend::message::{DisplayMessage, MessageExt, TextMessage};

use super::{message::ReactionMessage, Contact, Manager, Message};

const EMPTY_EMOJI: &String = &String::new();

gtk::glib::wrapper! {
    pub struct Channel(ObjectSubclass<imp::Channel>);
}

impl Channel {
    pub(super) async fn from_contact_or_group(
        contact: Contact,
        group_context: &Option<GroupContextV2>,
        manager: &Manager,
    ) -> Self {
        log::trace!("Trying to build a `Channel` from a `Contact` or `GroupContextV2`");
        let available_channels = manager.available_channels();
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .build();
        if let Some(group_context_v2) = group_context {
            if let Some(channel) = available_channels.iter().find(|c| {
                c.group_context().and_then(|c| c.master_key) == group_context_v2.master_key
            }) {
                return channel.clone();
            }

            // TODO: Can be `None`?
            let group = manager
                .get_group_v2(group_context_v2.master_key.clone().unwrap_or_default())
                .await;
            if let Ok(Some(group)) = group {
                return Self::from_group(group, group_context_v2, manager).await;
            } else {
                s.imp().contact.swap(&RefCell::new(Some(contact)));
            }
        } else {
            s.imp().contact.swap(&RefCell::new(Some(contact)));
        }
        s
    }

    pub(super) async fn from_group(
        group: Group,
        group_context_v2: &GroupContextV2,
        manager: &Manager,
    ) -> Self {
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .build();
        s.imp().group.swap(&RefCell::new(Some(group)));
        s.imp()
            .group_context
            .swap(&RefCell::new(Some(group_context_v2.clone())));
        s
    }

    fn thread(&self) -> Option<Thread> {
        if let Some(key) = self
            .group_context()
            .and_then(|c| c.master_key)
            .and_then(|k| <[u8; 32]>::try_from(k).ok())
        {
            Some(Thread::Group(key))
        } else {
            Some(Thread::Contact(self.uuid()?))
        }
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn last_message(&self) -> Option<Message> {
        self.property("last-message")
    }

    pub fn title(&self) -> String {
        self.property("title")
    }

    #[async_recursion::async_recursion(?Send)]
    pub async fn load_last(&self, number: usize) -> Vec<DisplayMessage> {
        if let Some(thread) = self.thread() {
            let mut results = vec![];
            let manager = self.manager();
            let messages_unborrowed = &self.imp().messages;
            let first_timestamp = {
                let msgs = messages_unborrowed.borrow();
                msgs.get(0).map(|m| m.sent() - 1)
            };
            crate::trace!(
                "Loading {} last messages for channel: {} (Thread: {:?}). Needed earlier then {:?}",
                number,
                self.title(),
                thread,
                first_timestamp
            );
            let iter = manager.messages(&thread, first_timestamp);
            if iter.is_err() {
                return vec![];
            }
            for content in iter.unwrap() {
                let msg = Message::from_content(content, &manager).await;
                if let Some(msg) = msg {
                    if let Some(msg) = msg.dynamic_cast_ref::<DisplayMessage>() {
                        {
                            let mut messages = messages_unborrowed.borrow_mut();
                            messages.insert(0, msg.clone());
                        }
                        results.insert(0, msg.clone());
                        self.notify("last-message");
                    }
                    let _ = self.do_new_message(&msg).await;
                }

                if results.len() == number {
                    break;
                }
            }
            results
        } else {
            vec![]
        }
    }

    pub(super) fn internal_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.imp().hash(&mut hasher);
        hasher.finish()
    }

    pub(super) fn group_context(&self) -> Option<GroupContextV2> {
        self.imp().group_context.borrow().clone()
    }

    fn uuid(&self) -> Option<Uuid> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .and_then(|c| c.address())
            .map(|a| a.uuid)
    }

    pub(super) async fn do_new_message(
        &self,
        message: &Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        if let Some(message) = message.dynamic_cast_ref::<TextMessage>() {
            if let Some(body) = message.property::<Option<String>>("body") {
                crate::trace!("Channel {} got new message: {}", self.title(), body);
                if let Some(quote) = message.quote().and_then(|q| q.id) {
                    log::trace!("Message claims to have a quote");
                    if let Some(thread) = self.thread() {
                        if let Ok(Some(quoted_msg)) = self.manager().message(&thread, quote).await {
                            if let Some(quoted_msg) = quoted_msg.dynamic_cast_ref::<TextMessage>() {
                                crate::trace!(
                                    "Message {} quotes other message {}",
                                    body,
                                    quoted_msg
                                        .property::<Option<String>>("body")
                                        .unwrap_or_default()
                                );
                                message.set_quote(quoted_msg);
                            }
                        }
                    }
                }
                let id = message.sent();
                if let Some(reactions) = self.imp().pending_reactions.borrow_mut().remove(&id) {
                    log::trace!("Adding pending reactions to message: {}", reactions);
                    message.react(reactions);
                }
            }
        }

        if let Some(reaction) = message
            .dynamic_cast_ref::<ReactionMessage>()
            .map(|r| r.reaction())
        {
            let reaction_emoji = reaction.emoji.as_ref().unwrap_or(EMPTY_EMOJI);
            crate::trace!(
                "Channel {} got new reaction: {}",
                self.title(),
                &reaction_emoji
            );
            let reacted_msg = self
                .messages()
                .into_iter()
                .find(|m| Some(m.sent()) == reaction.target_sent_timestamp);
            if let Some(reacted_msg) = reacted_msg {
                if let Some(reacted_msg) = reacted_msg.dynamic_cast_ref::<TextMessage>() {
                    crate::trace!(
                        "Reaction to message {}",
                        reacted_msg
                            .property::<Option<String>>("body")
                            .unwrap_or_default()
                    );
                    reacted_msg.react(reaction_emoji);
                } else {
                    log::warn!("Reaction message for a non-TextMessage");
                }
            } else {
                log::trace!("Message reacted to another message that could not be found yet. Inserting into pending reactions");
                let mut pending_reactions = self.imp().pending_reactions.borrow_mut();
                let entry = pending_reactions
                    .entry(
                        reaction
                            .target_sent_timestamp
                            .expect("Reacted message to have timestamp"),
                    )
                    .or_insert_with(|| "".to_string());
                entry.push_str(reaction_emoji);
            }
        }
        Ok(())
    }

    pub(super) async fn new_message(
        &self,
        message: Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        log::trace!("Adding new message to channel");

        // Check if message is duplicate
        if self.messages().iter().rev().any(|m| {
            message.sent() == m.sent()
                && message.sender().address().map(|a| a.uuid)
                    == m.sender().address().map(|a| a.uuid)
        }) {
            crate::info!(
                "Channel {} got a duplicate message. Ignoring the second one.",
                self.title()
            );
            return Ok(());
        }

        self.do_new_message(&message).await?;
        if let Some(message) = message.dynamic_cast_ref::<DisplayMessage>() {
            let mut msgs = self.imp().messages.borrow_mut();
            let insert_pos = msgs
                .binary_search_by_key(&message.sent(), |m| m.sent())
                .expect_err("Message already exists in binary search, but did not exist before");
            msgs.insert(insert_pos, message.clone());
            drop(msgs);
            self.notify("last-message");
            message.send_notification();
            self.emit_by_name::<()>("message", &[&message]);
        } else {
            log::trace!("Channel skip adding empty message");
        }
        Ok(())
    }

    pub fn messages(&self) -> Vec<DisplayMessage> {
        self.imp().messages.borrow().clone()
    }

    pub fn previous_message_to(&self, msg: &DisplayMessage) -> Option<DisplayMessage> {
        let messages = self.messages();
        let idx = messages.iter().position(|m| {
            m.sent() == msg.sent()
                && m.property::<Option<String>>("body") == msg.property::<Option<String>>("body")
        })?;
        if idx == 0 {
            None
        } else {
            Some(messages[idx - 1].clone())
        }
    }

    pub(super) async fn send_internal_message(
        &self,
        mut data: DataMessage,
        timestamp: u64,
    ) -> Result<(), crate::ApplicationError> {
        let manager = self.manager();
        let receiver_contact = self
            .imp()
            .contact
            .borrow()
            .as_ref()
            .and_then(|c| c.address());

        if let Some(contact) = receiver_contact {
            log::trace!("Sending to single contact");
            manager.send_message(contact, data, timestamp).await?;
        } else {
            let context = self.imp().group_context.borrow().clone();
            data.group_v2 = context.clone();
            // TODO: Can this be `None`?
            if let Some(key) = context.as_ref().and_then(|c| c.master_key.clone()) {
                manager.send_message_to_group(key, data, timestamp).await?;
            }
        }
        Ok(())
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), crate::ApplicationError> {
        if let Some(msg) = msg.dynamic_cast_ref::<DisplayMessage>() {
            self.imp().messages.borrow_mut().push(msg.clone());
        }

        self.notify("last-message");
        self.emit_by_name::<()>("message", &[&msg]);

        crate::debug!(
            "Sending a message {} to channel {}",
            msg.property::<Option<String>>("body")
                .unwrap_or_else(|| "(empty)".to_owned()),
            self.title()
        );
        if let Some(data) = msg.internal_data() {
            self.send_internal_message(data, msg.sent()).await?;
        }
        Ok(())
    }
}

mod imp {
    use std::{cell::RefCell, collections::HashMap};

    use gdk::{prelude::*, subclass::prelude::*};
    use glib::{
        once_cell::sync::Lazy, subclass::Signal, ParamSpec, ParamSpecObject, ParamSpecString, Value,
    };
    use gtk::{gdk, glib};
    use libsignal_service::groups_v2::Group;
    use presage::prelude::{GroupContextV2, Uuid};

    use crate::backend::{message::DisplayMessage, Contact, Manager};

    #[derive(Default)]
    pub struct Channel {
        pub(super) contact: RefCell<Option<Contact>>,
        pub(super) group: RefCell<Option<Group>>,
        pub(super) group_context: RefCell<Option<GroupContextV2>>,

        pub(super) manager: RefCell<Option<Manager>>,
        pub(super) messages: RefCell<Vec<DisplayMessage>>,
        pub(super) pending_reactions: RefCell<HashMap<u64, String>>,
    }

    impl std::hash::Hash for Channel {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            if let Some(uuid) = self
                .contact
                .borrow()
                .as_ref()
                .and_then(|c| c.address())
                .map(|a| a.uuid)
            {
                uuid.hash(state);
            } else {
                None::<Uuid>.hash(state)
            }
            if let Some(uuids) = self
                .group
                .borrow()
                .as_ref()
                .map(|g| &g.members)
                .map(|m| m.iter().map(|c| c.uuid).collect::<Vec<Uuid>>())
            {
                uuids.hash(state);
            } else {
                None::<Uuid>.hash(state)
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Channel {
        const NAME: &'static str = "FlChannel";
        type Type = super::Channel;
    }

    impl ObjectImpl for Channel {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<DisplayMessage>("last-message")
                        .read_only()
                        .build(),
                    ParamSpecString::builder("title").read_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "last-message" => self.messages.borrow().last().to_value(),
                "title" => {
                    let title = if let Some(group) = self.group.borrow().as_ref() {
                        group.title.clone()
                    } else if let Some(contact) = self.contact.borrow().as_ref() {
                        if contact.property::<bool>("is-self") {
                            gettextrs::gettext("Note to self")
                        } else {
                            contact.title()
                        }
                    } else {
                        "".to_string()
                    };

                    title.to_value()
                }
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Channel` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("message")
                    .param_types([DisplayMessage::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }
}
