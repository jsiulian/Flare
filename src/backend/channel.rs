use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gdk_pixbuf::{glib::Object, prelude::ObjectExt};
use gio::subclass::prelude::ObjectSubclassIsExt;
use libsignal_service::{groups_v2::Group, proto::DataMessage};
use presage::{
    prelude::{GroupContextV2, GroupMasterKey, ServiceAddress},
    MessageIdentity,
};

use super::{Contact, Manager, Message};

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
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Channel`");
        if let Some(group_context_v2) = group_context {
            let master_key = GroupMasterKey::new(
                group_context_v2
                    .master_key
                    .clone()
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            if let Some(channel) = available_channels.iter().find(|c| {
                c.group_context().and_then(|c| c.master_key) == group_context_v2.master_key
            }) {
                return channel.clone();
            }

            let group = manager.get_group_v2(master_key).await;
            if let Ok(group) = group {
                return Self::from_group(group, group_context_v2, manager).await;
            } else {
                if let Some(ref uuid) = contact.address().and_then(|a| a.uuid) {
                    s.imp().unloaded_messages.swap(&RefCell::new(
                        manager.messages_by_contact(uuid).unwrap_or_default(),
                    ));
                }
                s.imp().contact.swap(&RefCell::new(Some(contact)));
            }
        } else {
            if let Some(ref uuid) = contact.address().and_then(|a| a.uuid) {
                s.imp().unloaded_messages.swap(&RefCell::new(
                    manager.messages_by_contact(uuid).unwrap_or_default(),
                ));
            }

            s.imp().contact.swap(&RefCell::new(Some(contact)));
        }
        s
    }

    pub(super) async fn from_group(
        group: Group,
        group_context_v2: &GroupContextV2,
        manager: &Manager,
    ) -> Self {
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Channel`");
        s.imp().group.swap(&RefCell::new(Some(group)));
        s.imp().unloaded_messages.swap(&RefCell::new(
            manager
                .messages_by_group(group_context_v2)
                .unwrap_or_default(),
        ));
        s.imp()
            .group_context
            .swap(&RefCell::new(Some(group_context_v2.clone())));
        s.imp().unloaded_messages.swap(&RefCell::new(
            manager
                .messages_by_group(group_context_v2)
                .unwrap_or_default(),
        ));
        s
    }

    #[async_recursion::async_recursion(?Send)]
    pub async fn load_last(&self, number: usize) -> Vec<Message> {
        log::trace!(
            "Loading last messages for contact: {}",
            self.property::<String>("title")
        );
        let to_load = {
            let mut unloaded_msgs = self.imp().unloaded_messages.borrow_mut();
            if unloaded_msgs.len() <= number {
                unloaded_msgs.drain(..).collect::<Vec<_>>()
            } else {
                unloaded_msgs.drain(..number).collect::<Vec<_>>()
            }
        };

        if to_load.is_empty() {
            return vec![];
        }

        let mut empty = 0;
        let mut results = vec![];
        let manager = self.property::<Manager>("manager");
        for msg_id in to_load {
            if let Ok(Some(msg)) = manager.message_by_id(&msg_id).await {
                if !msg.is_empty() {
                    {
                        let mut messages = self.imp().messages.borrow_mut();
                        messages.insert(0, msg.clone());
                    }
                    results.insert(0, msg.clone());
                    self.notify("last-message");
                } else {
                    empty += 1
                }
                // TODO: Error?
                let _ = self.do_new_message(&msg).await;
            }
        }

        let mut previous = if empty != 0 {
            self.load_last(empty).await
        } else {
            vec![]
        };
        previous.append(&mut results);
        previous
    }

    pub(super) fn internal_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.imp().hash(&mut hasher);
        hasher.finish()
    }

    pub(super) fn group_context(&self) -> Option<GroupContextV2> {
        self.imp().group_context.borrow().clone()
    }

    pub(super) async fn do_new_message(
        &self,
        message: &Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        if let Some(body) = message.property::<Option<String>>("body") {
            crate::trace!(
                "Channel {} got new message: {}",
                self.property::<String>("title"),
                body
            );
            if let Some(quote) = message.quote() {
                if let Ok(id) = MessageIdentity::try_from(&quote) {
                    if let Ok(Some(quoted_msg)) =
                        self.property::<Manager>("manager").message_by_id(&id).await
                    {
                        crate::trace!(
                            "Message {} quotes other message {}",
                            body,
                            quoted_msg
                                .property::<Option<String>>("body")
                                .unwrap_or_else(|| "".to_string())
                        );
                        message.set_quote(quoted_msg);
                    }
                }
            }
            if let Some(id) = message.id() {
                if let Some(reactions) = self.imp().pending_reactions.borrow_mut().remove(&id) {
                    log::trace!("Adding pending reactions to message: {}", reactions);
                    message.react(reactions);
                }
            }
        }
        if let Some(reaction) = message.reaction() {
            let reaction_emoji = reaction.emoji.as_ref().unwrap_or(EMPTY_EMOJI);
            crate::trace!(
                "Channel {} got new reaction: {}",
                self.property::<String>("title"),
                &reaction_emoji
            );
            let reacted_msg = self
                .messages()
                .into_iter()
                .find(|m| m.timestamp() == reaction.target_sent_timestamp);
            if let Some(reacted_msg) = reacted_msg {
                crate::trace!(
                    "Reaction to message {}",
                    reacted_msg
                        .property::<Option<String>>("body")
                        .unwrap_or_else(|| "".to_string())
                );
                reacted_msg.react(&reaction_emoji);
            } else {
                crate::trace!("Message reacted to another message that could not be found yet. Inserting into pending reactions", );
                let mut pending_reactions = self.imp().pending_reactions.borrow_mut();
                let entry = pending_reactions
                    .entry(
                        (&reaction)
                            .try_into()
                            .expect("Reacted message to have UUID"),
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
        self.do_new_message(&message).await?;
        if !message.is_empty() {
            self.imp().messages.borrow_mut().push(message.clone());
            self.notify("last-message");
            self.try_emit_by_name::<()>("message", &[&message])?;
        }
        Ok(())
    }

    pub fn messages(&self) -> Vec<Message> {
        self.imp().messages.borrow().clone()
    }

    pub fn previous_message_to(&self, msg: &Message) -> Option<Message> {
        let messages = self.messages();
        let idx = messages.iter().position(|m| {
            m.timestamp() == msg.timestamp()
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
        let manager = self.property::<Manager>("manager");
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
            {
                let context = self.imp().group_context.borrow();
                data.group_v2 = context.clone();
            }
            let receiver_group_addresses = if let Some(group) = self.imp().group.borrow().as_ref() {
                group
                    .members
                    .iter()
                    .map(|m| m.uuid)
                    .map(|u| manager.get_contact_by_id(u))
                    .filter(|u| matches!(u, Ok(Some(_))))
                    .map(|c| c.expect("Match Failed").expect("Match Failed").address)
                    .collect::<Vec<ServiceAddress>>()
            } else {
                return Ok(());
            };
            manager
                .send_message_to_group(receiver_group_addresses, data, timestamp)
                .await?;
        }
        Ok(())
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), crate::ApplicationError> {
        self.imp().messages.borrow_mut().push(msg.clone());
        self.notify("last-message");
        self.emit_by_name::<()>("message", &[&msg]);

        crate::debug!(
            "Sending a message {} to channel {}",
            msg.property::<Option<String>>("body")
                .unwrap_or_else(|| "(empty)".to_owned()),
            self.property::<String>("title")
        );
        if let Some(data) = msg.data() {
            self.send_internal_message(
                data,
                msg.timestamp().expect("Messate to send to have timestamp"),
            )
            .await?;
        }
        Ok(())
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::{
        glib::{
            once_cell::sync::Lazy, subclass::Signal, ParamFlags, ParamSpec, ParamSpecObject,
            ParamSpecString, Value,
        },
        prelude::{ObjectExt, StaticType, ToValue},
    };
    use gtk::glib;
    use presage::{
        libsignal_service::groups_v2::Group,
        prelude::{GroupContextV2, Uuid},
        MessageIdentity,
    };
    use std::{cell::RefCell, collections::HashMap};

    use crate::backend::{Contact, Manager, Message};

    #[derive(Default)]
    pub struct Channel {
        pub(super) contact: RefCell<Option<Contact>>,
        pub(super) group: RefCell<Option<Group>>,
        pub(super) group_context: RefCell<Option<GroupContextV2>>,

        pub(super) manager: RefCell<Option<Manager>>,
        pub(super) messages: RefCell<Vec<Message>>,
        pub(super) unloaded_messages: RefCell<Vec<MessageIdentity>>,
        pub(super) pending_reactions: RefCell<HashMap<MessageIdentity, String>>,
    }

    impl std::hash::Hash for Channel {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            if let Some(uuid) = self
                .contact
                .borrow()
                .as_ref()
                .and_then(|c| c.address())
                .and_then(|a| a.uuid)
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
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecObject::new(
                        "last-message",
                        "last-message",
                        "last-message",
                        Message::static_type(),
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new("title", "title", "title", None, ParamFlags::READABLE),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
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
                            contact.property::<String>("title")
                        }
                    } else {
                        "".to_string()
                    };

                    title.to_value()
                }
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
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
                vec![Signal::builder(
                    "message",
                    &[Message::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });
            SIGNALS.as_ref()
        }
    }
}
