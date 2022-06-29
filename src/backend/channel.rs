use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gdk_pixbuf::{glib::Object, prelude::ObjectExt};
use gio::subclass::prelude::ObjectSubclassIsExt;
use libsignal_service::proto::DataMessage;
use presage::prelude::{GroupContextV2, GroupMasterKey, ServiceAddress};

use super::{Contact, Manager, Message};

gtk::glib::wrapper! {
    pub struct Channel(ObjectSubclass<imp::Channel>);
}

impl Channel {
    pub(super) async fn from_contact_or_group(
        contact: Contact,
        group_context: &Option<GroupContextV2>,
        manager: &Manager,
    ) -> Self {
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
            let group = manager.internal().get_group_v2(master_key).await;
            if let Ok(group) = group {
                s.imp().group.swap(&RefCell::new(Some(group)));
                s.imp()
                    .group_context
                    .swap(&RefCell::new(Some(group_context_v2.clone())));
            } else {
                s.imp().contact.swap(&RefCell::new(Some(contact)));
            }
        } else {
            s.imp().contact.swap(&RefCell::new(Some(contact)));
        }
        return s;
    }

    pub(super) fn internal_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.imp().hash(&mut hasher);
        return hasher.finish();
    }

    pub(super) fn new_message(&self, message: Message) {
        if let Some(body) = message.property::<Option<String>>("body") {
            crate::trace!(
                "Channel {} got new message: {}",
                self.property::<String>("title"),
                body
            );
            if let Some(quote_timestamp) = message.quote_timestamp() {
                let quoted_msg = self
                    .messages()
                    .into_iter()
                    .filter(|m| m.timestamp() == Some(quote_timestamp))
                    .next();
                if let Some(quoted_msg) = quoted_msg {
                    crate::trace!(
                        "Message {} quotes other message {}",
                        body,
                        quoted_msg
                            .property::<Option<String>>("body")
                            .unwrap_or("".to_string())
                    );
                    message.set_quote(quoted_msg);
                } else {
                    crate::warn!("Message quotes another message that could not be found",);
                }
            }
            self.imp().messages.borrow_mut().push(message.clone());
            self.notify("last-message");
            self.emit_by_name::<()>("message", &[&message]);
        }
        if let Some(reaction) = message.reaction() {
            let reaction_emoji = reaction.emoji.unwrap_or("".to_string());
            crate::trace!(
                "Channel {} got new reaction: {}",
                self.property::<String>("title"),
                &reaction_emoji
            );
            let reacted_msg = self
                .messages()
                .into_iter()
                .filter(|m| m.timestamp() == reaction.target_sent_timestamp)
                .next();
            if let Some(reacted_msg) = reacted_msg {
                crate::trace!(
                    "Reaction to message {}",
                    reacted_msg
                        .property::<Option<String>>("body")
                        .unwrap_or("".to_string())
                );
                reacted_msg.react(&reaction_emoji);
            } else {
                crate::warn!("Message reacted to another message that could not be found",);
            }
        }
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
            return None;
        } else {
            return Some(messages[idx - 1].clone());
        }
    }

    pub(super) async fn send_internal_message(&self, mut data: DataMessage, timestamp: u64) {
        let manager = self.property::<Manager>("manager").internal();
        let receiver_contact = self
            .imp()
            .contact
            .borrow()
            .as_ref()
            .map(|c| c.address())
            .flatten();
        let receiver_group = self.imp().group.borrow();

        if let Some(contact) = receiver_contact {
            log::trace!("Sending to single contact");
            // TODO: Error Handling
            let _ = manager.send_message(contact, data, timestamp).await;
        } else if let Some(group) = receiver_group.as_ref() {
            let context = self.imp().group_context.borrow();
            // TODO: Error Handling
            data.group_v2 = context.clone();
            let receiver_group_addresses = group
                .members
                .iter()
                .map(|m| m.uuid)
                .map(|u| manager.get_contact_by_id(u))
                .filter(|u| matches!(u, Ok(Some(_))))
                .map(|c| c.expect("Match Failed").expect("Match Failed").address)
                .collect::<Vec<ServiceAddress>>();
            let _ = manager
                .send_message_to_group(receiver_group_addresses, data, timestamp)
                .await;
        }
    }

    pub async fn send_message(&self, msg: Message) {
        self.imp().messages.borrow_mut().push(msg.clone());
        self.notify("last-message");
        self.emit_by_name::<()>("message", &[&msg]);

        crate::debug!(
            "Sending a message {} to channel {}",
            msg.property::<String>("body"),
            self.property::<String>("title")
        );
        if let Some(data) = msg.data() {
            self.send_internal_message(
                data,
                msg.timestamp().expect("Messate to send to have timestamp"),
            )
            .await;
        }
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
    };
    use std::cell::RefCell;

    use crate::backend::{Contact, Manager, Message};

    #[derive(Default)]
    pub struct Channel {
        pub(super) contact: RefCell<Option<Contact>>,
        pub(super) group: RefCell<Option<Group>>,
        pub(super) group_context: RefCell<Option<GroupContextV2>>,

        pub(super) manager: RefCell<Option<Manager>>,
        pub(super) messages: RefCell<Vec<Message>>,
    }

    impl std::hash::Hash for Channel {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            if let Some(uuid) = self
                .contact
                .borrow()
                .as_ref()
                .map(|c| c.address())
                .flatten()
                .map(|a| a.uuid)
                .flatten()
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
