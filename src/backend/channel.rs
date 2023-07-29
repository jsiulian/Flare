use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gdk::{
    glib::clone,
    prelude::{ObjectExt, SettingsExt},
};
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{Cast, Object};
use gtk::{gdk, gio, glib};
use libsignal_service::{groups_v2::Group, ServiceAddress};
use presage::{
    prelude::{DataMessage, GroupContextV2, Uuid},
    Thread,
};

use crate::{
    backend::{
        message::{DisplayMessage, MessageExt, TextMessage},
        timeline::{TimelineItem, TimelineItemExt},
    },
    ApplicationError,
};

use super::{
    message::{DeletionMessage, ReactionMessage},
    Contact, Manager, Message,
};

gtk::glib::wrapper! {
    pub struct Channel(ObjectSubclass<imp::Channel>);
}

const EMPTY_MESSAGE_BODY: &str = "<empty>";

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
                contact.connect_notify_local(
                    Some("title"),
                    clone!(@weak s => move |_, _| s.notify("title")),
                );
                s.imp().contact.swap(&RefCell::new(Some(contact)));
            }
        } else {
            contact.connect_notify_local(
                Some("title"),
                clone!(@weak s => move |_, _| s.notify("title")),
            );
            s.imp().contact.swap(&RefCell::new(Some(contact)));
        }
        s.initialize_participants().await;
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
        s.initialize_participants().await;
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

    pub async fn load_last(&self, number: usize) {
        if let Some(thread) = self.thread() {
            let mut results = vec![];
            let manager = self.manager();
            let first_timestamp = self
                .imp()
                .timeline
                .borrow()
                .iter_forwards()
                .filter(|i| i.is::<DisplayMessage>())
                .map(|m| m.timestamp())
                .next();
            crate::trace!(
                "Loading {} last messages for channel: {} (Thread: {:?}). Needed earlier then {:?}",
                number,
                self.title(),
                thread,
                first_timestamp
            );
            let iter = manager.messages(&thread, first_timestamp).await;
            if iter.is_err() {
                log::error!("Failed to load last messages: {}", iter.err().unwrap());
                return;
            }
            for content in iter.unwrap() {
                let msg = Message::from_content(content, &manager).await;
                if let Some(msg) = msg {
                    if let Some(msg) = msg.dynamic_cast_ref::<DisplayMessage>() {
                        results.insert(0, msg.clone());
                    }

                    let _ = self.do_new_message(&msg).await;
                }

                if results.len() == number {
                    break;
                }
            }

            self.imp().timeline.borrow().prepend(
                results
                    .into_iter()
                    .map(|i| {
                        i.dynamic_cast::<TimelineItem>()
                            .expect("A 'DisplayMessage' to be a 'TimelineItem'")
                    })
                    .collect(),
            );
            self.notify("last-message");
        }
    }

    pub fn trim_old(&self) {
        self.imp().timeline.borrow().trim_old()
    }

    pub(super) fn internal_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.imp().hash(&mut hasher);
        hasher.finish()
    }

    pub(super) fn group_context(&self) -> Option<GroupContextV2> {
        self.imp().group_context.borrow().clone()
    }

    pub fn group(&self) -> Option<Group> {
        self.imp().group.borrow().clone()
    }

    pub fn uuid(&self) -> Option<Uuid> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .and_then(|c| c.address())
            .map(|a| a.uuid)
    }

    pub async fn send_session_reset(&self) -> Result<(), ApplicationError> {
        log::trace!("Sending session reset");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;
        let Some(uuid) = self.uuid() else {return Ok(())};
        self.manager().send_session_reset(uuid, ts).await
    }

    pub(super) async fn do_new_message(
        &self,
        message: &Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        if let Some(message) = message.dynamic_cast_ref::<TextMessage>() {
            let body = message
                .body()
                .unwrap_or_else(|| String::from(EMPTY_MESSAGE_BODY));
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
            let id = message.timestamp();
            if let Some(reactions) = self.imp().pending_reactions.borrow_mut().remove(&id) {
                log::trace!(
                    "Adding pending reactions to message: {:?}",
                    reactions.iter().map(|r| r.emoji()).collect::<Vec<_>>()
                );
                for reaction in reactions {
                    message.react(&reaction);
                }
            }
        }

        if let Some(reaction) = message.dynamic_cast_ref::<ReactionMessage>() {
            crate::trace!(
                "Channel {} got new reaction: {}",
                self.title(),
                reaction.emoji()
            );

            let reacted_msg = self
                .imp()
                .timeline
                .borrow()
                .get_by_timestamp(reaction.target_timestamp())
                .and_then(|o| o.dynamic_cast::<DisplayMessage>().ok());

            if let Some(reacted_msg) = reacted_msg {
                if let Some(reacted_msg) = reacted_msg.dynamic_cast_ref::<TextMessage>() {
                    crate::trace!(
                        "Reaction to message {}",
                        reacted_msg
                            .property::<Option<String>>("body")
                            .unwrap_or_default()
                    );
                    reacted_msg.react(reaction);
                } else {
                    log::warn!("Reaction message for a non-TextMessage");
                }
            } else {
                log::trace!("Message reacted to another message that could not be found yet. Inserting into pending reactions");
                let mut pending_reactions = self.imp().pending_reactions.borrow_mut();
                let entry = pending_reactions
                    .entry(reaction.target_timestamp())
                    .or_insert_with(Vec::new);
                let to_insert = entry
                    .binary_search_by_key(&reaction.timestamp(), |m| m.timestamp())
                    .unwrap_or_else(|e| e);
                entry.insert(to_insert, reaction.clone());
            }
        }

        if let Some(deletion) = message
            .dynamic_cast_ref::<DeletionMessage>()
            .map(|r| r.deletion())
        {
            crate::trace!(
                "Channel {} got a delete message for timestamp: {}",
                self.title(),
                &deletion.target_sent_timestamp()
            );

            if deletion.target_sent_timestamp.is_none() {
                log::warn!("Got deletion message but without a sent timestamp. Aborting deletion.");
                return Ok(());
            }

            let deleted_msg = self
                .imp()
                .timeline
                .borrow()
                .get_by_timestamp(deletion.target_sent_timestamp.unwrap())
                .and_then(|o| o.dynamic_cast::<DisplayMessage>().ok());
            if let Some(deleted_msg) = deleted_msg {
                if let Some(deleted_msg) = deleted_msg.dynamic_cast_ref::<TextMessage>() {
                    crate::trace!(
                        "Deletion to message {}",
                        deleted_msg
                            .property::<Option<String>>("body")
                            .unwrap_or_default()
                    );
                    deleted_msg.mark_as_deleted();
                    self.notify("last-message");
                } else {
                    log::warn!("Deletion message for a non-TextMessage");
                }
            } else {
                log::trace!("Deletion message aimed at a unloaded message. Will be ignored");
            }
        }
        Ok(())
    }

    pub(super) async fn new_message(
        &self,
        message: Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        log::trace!("Adding new message to channel");
        self.do_new_message(&message).await?;
        if let Some(message) = message.dynamic_cast_ref::<DisplayMessage>() {
            self.imp().timeline.borrow().append(
                message
                    .clone()
                    .dynamic_cast::<TimelineItem>()
                    .expect("A 'DisplayMessage' to be a 'TimelineItem'"),
            );

            self.notify("last-message");
            message.send_notification();
            self.emit_by_name::<()>("message", &[&message]);
        } else if let Some(message) = message.dynamic_cast_ref::<ReactionMessage>() {
            if self.manager().settings().boolean("notify-reactions") {
                message.send_notification();
            }
        } else {
            log::trace!("Channel skip adding empty message");
        }
        Ok(())
    }

    pub fn messages(&self) -> Vec<DisplayMessage> {
        // self.imp().messages.borrow().clone()
        vec![]
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
        crate::debug!(
            "Sending a message {} to channel {} (timestamp {})",
            msg.property::<Option<String>>("body")
                .unwrap_or_else(|| "(empty)".to_owned()),
            self.title(),
            msg.timestamp()
        );
        if let Some(data) = msg.internal_data() {
            self.send_internal_message(data, msg.timestamp()).await?;
        }

        log::trace!("Inserting successfully sent message to message list");
        if let Some(msg) = msg.dynamic_cast_ref::<DisplayMessage>() {
            self.imp().timeline.borrow().append(
                msg.clone()
                    .dynamic_cast::<TimelineItem>()
                    .expect("A 'DisplayMessage' to be a 'TimelineItem'"),
            );
        }

        self.notify("last-message");
        self.emit_by_name::<()>("message", &[&msg]);

        Ok(())
    }

    pub fn participants(&self) -> Vec<Contact> {
        self.imp().participants.borrow().clone()
    }

    pub fn participant_by_uuid(&self, uuid: Uuid) -> Contact {
        let found = self.participants().into_iter().find(|c| c.uuid() == uuid);
        if let Some(found) = found {
            return found;
        }
        let new = Contact::from_service_address(&ServiceAddress { uuid }, &self.manager());
        self.imp().participants.borrow_mut().push(new.clone());
        new
    }

    async fn initialize_participants(&self) {
        let manager = self.manager();
        if let Some(group) = self.group() {
            let participants = group
                .members
                .into_iter()
                .map(|m| ServiceAddress { uuid: m.uuid })
                .map(|a| Contact::from_service_address(&a, &manager))
                .collect::<Vec<_>>();
            for p in &participants {
                p.set_channel(Some(self));
            }
            self.imp().participants.replace(participants);
        } else {
            let participants = self
                .imp()
                .contact
                .borrow()
                .as_ref()
                .cloned()
                .into_iter()
                .collect::<Vec<_>>();
            for p in &participants {
                p.set_channel(Some(self));
            }
            self.imp().participants.replace(participants);
        }
    }
}

mod imp {
    use std::{cell::RefCell, collections::HashMap};

    use gdk::{glib::ParamSpecBoolean, prelude::*, subclass::prelude::*};
    use glib::{
        once_cell::sync::Lazy, subclass::Signal, ParamSpec, ParamSpecObject, ParamSpecString, Value,
    };
    use gtk::{gdk, glib};
    use libsignal_service::groups_v2::Group;
    use presage::prelude::{GroupContextV2, Uuid};

    use crate::backend::{
        message::{DisplayMessage, ReactionMessage, TextMessage},
        timeline::Timeline,
        Contact, Manager,
    };

    #[derive(Default)]
    pub struct Channel {
        pub(super) contact: RefCell<Option<Contact>>,
        pub(super) group: RefCell<Option<Group>>,
        pub(super) group_context: RefCell<Option<GroupContextV2>>,

        pub(super) participants: RefCell<Vec<Contact>>,

        pub(super) manager: RefCell<Option<Manager>>,
        pub(super) timeline: RefCell<Timeline>,
        pub(super) pending_reactions: RefCell<HashMap<u64, Vec<ReactionMessage>>>,
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
                    ParamSpecObject::builder::<Timeline>("timeline")
                        .read_only()
                        .build(),
                    ParamSpecObject::builder::<DisplayMessage>("last-message")
                        .read_only()
                        .build(),
                    ParamSpecString::builder("title").read_only().build(),
                    ParamSpecBoolean::builder("is-contact").read_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "timeline" => self.timeline.borrow().to_value(),
                "last-message" => self
                    .timeline
                    .borrow()
                    .iter_backwards()
                    .filter(|i| i.is::<DisplayMessage>())
                    .map(|m| m.dynamic_cast::<DisplayMessage>().unwrap())
                    .find(|m| {
                        !(*m)
                            .clone()
                            .downcast::<TextMessage>()
                            .map(|m| m.is_deleted())
                            .unwrap_or(false)
                    })
                    .to_value(),
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
                "is-contact" => self.contact.borrow().as_ref().is_some().to_value(),
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
