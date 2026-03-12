use crate::backend::{
    Contact, Manager, Message,
    message::{
        DeletionMessage, DisplayMessage, DisplayMessageExt, MessageExt, ReactionMessage,
        TextMessage,
    },
    timeline::{TimelineItem, TimelineItemExt},
};
use crate::prelude::*;

use std::hash::Hash;

use gdk::Texture;
use glib::Bytes;
use glib::{Object, prelude::Cast};

use libsignal_service::{
    proto::{DataMessage, GroupContextV2},
    protocol::ServiceId,
};
use presage::model::groups::Group;
use presage::store::Thread;

gtk::glib::wrapper! {
    /// A channel represents either a 1-to-1 channel or a group.
    pub struct Channel(ObjectSubclass<imp::Channel>);
}

const EMPTY_MESSAGE_BODY: &str = "<empty>";
const TYPING_NOTIFICATION_DURATION_SECONDS: u32 = 10;

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct TypingNotification {
    pub sender: Contact,
    pub timestamp: u64,
}

impl Channel {
    pub(super) async fn from_contact_or_group(
        contact: Contact,
        group_context: &Option<GroupContextV2>,
        manager: &Manager,
    ) -> Self {
        log::trace!("Trying to build a `Channel` from a `Contact` or `GroupContextV2`");

        // Use cached channel if available.
        let available_channels = manager.available_channels();
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .build();

        // Set from a group.
        if let Some(group_context_v2) = group_context {
            if let Some(channel) = available_channels.iter().find(|c| {
                c.group_context().and_then(|c| c.master_key) == group_context_v2.master_key
            }) {
                return channel.clone();
            }

            // TODO: Can be `None`?
            // XXX: Error on invalid size?
            let group = manager
                .get_group_v2(
                    group_context_v2
                        .master_key
                        .clone()
                        .unwrap_or_default()
                        .try_into()
                        .unwrap_or_default(),
                )
                .await;
            if let Ok(Some(group)) = group {
                return Self::from_group(group, group_context_v2, manager).await;
            } else {
                contact.connect_notify_local(
                    Some("title"),
                    clone!(
                        #[weak]
                        s,
                        move |_, _| s.notify("title")
                    ),
                );
                s.imp().contact.swap(&RefCell::new(Some(contact)));
            }
        } else {
            // Set from a contact.
            contact.connect_notify_local(
                Some("title"),
                clone!(
                    #[weak]
                    s,
                    move |_, _| s.notify("title")
                ),
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

    pub fn thread(&self) -> Thread {
        if let Some(key) = self
            .group_context()
            .and_then(|c| c.master_key)
            .and_then(|k| <[u8; 32]>::try_from(k).ok())
        {
            Thread::Group(key)
        } else {
            Thread::Contact(
                self.uuid()
                    .expect("Channel to either have a group key or have a UUID"),
            )
        }
    }

    /// Load the specified number of message from the end of the message queue.
    pub async fn load_last(&self, number: usize) {
        let thread = self.thread();
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

                // Mark message from storage as already read.
                // TODO: Mark as read instead using sync messages, and store read state in DB.
                msg.mark_as_read();

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

    pub fn trim_old(&self) {
        self.imp().timeline.borrow().trim_old()
    }

    pub async fn clear_messages(&self) -> Result<(), ApplicationError> {
        self.manager().clear_channel_messages(self).await?;
        self.imp().timeline.borrow().clear();
        Ok(())
    }

    pub(super) fn group_context(&self) -> Option<GroupContextV2> {
        self.imp().group_context.borrow().clone()
    }

    pub fn group(&self) -> std::cell::Ref<'_, Option<Group>> {
        self.imp().group.borrow()
    }

    pub fn contact(&self) -> Option<Contact> {
        self.imp().contact.borrow().clone()
    }

    pub fn uuid(&self) -> Option<ServiceId> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .and_then(|c| c.address())
    }

    pub async fn send_session_reset(&self) -> Result<(), ApplicationError> {
        log::trace!("Sending session reset");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;
        let Some(uuid) = self.uuid() else {
            return Ok(());
        };
        self.manager().send_session_reset(uuid, ts).await
    }

    /// Register a new message with the channel.
    /// This does the following (based on the type of message):
    /// - Add a quote to the message if needed.
    /// - Cache pending reactions and apply them for the correct message.
    /// - Delete a message in the current channel.
    /// - Mark the message as read if the current channel is active.
    pub(super) async fn do_new_message(
        &self,
        message: &Message,
    ) -> Result<(), gtk::glib::error::BoolError> {
        if self.property("is-active") || message.sender().is_self() {
            message.mark_as_read();
        }

        if let Some(message) = message.dynamic_cast_ref::<TextMessage>() {
            let body = message
                .body()
                .unwrap_or_else(|| String::from(EMPTY_MESSAGE_BODY));

            // Message quote.
            crate::trace!("Channel {} got new message: {}", self.title(), body);
            if let Some(quote) = message.quote_timestamp() {
                log::trace!("Message claims to have a quote");
                let thread = self.thread();
                if let Ok(Some(quoted_msg)) = self.manager().message(&thread, quote).await
                    && let Some(quoted_msg) = quoted_msg.dynamic_cast_ref::<TextMessage>()
                {
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

            // Apply pending reactions.
            let id = message.timestamp();
            if let Some(reactions) = self.imp().pending_reactions.borrow_mut().get(&id) {
                log::trace!(
                    "Adding pending reactions to message: {:?}",
                    reactions.iter().map(|r| r.emoji()).collect::<Vec<_>>()
                );
                for reaction in reactions {
                    message.react(reaction);
                }
            }
        }

        // Apply reactions or store them.
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
                log::trace!(
                    "Message reacted to another message that could not be found yet. Inserting into pending reactions"
                );
                let mut pending_reactions = self.imp().pending_reactions.borrow_mut();
                let entry = pending_reactions
                    .entry(reaction.target_timestamp())
                    .or_default();
                let to_insert = entry
                    .binary_search_by_key(&reaction.timestamp(), |m| m.timestamp())
                    .unwrap_or_else(|e| e);
                entry.insert(to_insert, reaction.clone());
            }
        }

        // Delete messages.
        // Note we do not need to store pending deletion requests, those will not be returned by presage after deletion.
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

    /// Add a message to the channel, which includes putting it into the timeline, notifying it and updating the last message.
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
            if !message.property::<bool>("read") {
                message.send_notification();
            }
            self.emit_by_name::<()>("message", &[&message]);
        } else if let Some(message) = message.dynamic_cast_ref::<ReactionMessage>() {
            if self.manager().settings().boolean("notify-reactions")
                && !message.property::<bool>("read")
            {
                message.send_notification();
            }
        } else {
            log::trace!("Channel skip adding empty message");
        }
        Ok(())
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
            data.group_v2.clone_from(&context);
            // TODO: Can this be `None`?
            if let Some(key) = context.as_ref().and_then(|c| c.master_key.clone()) {
                manager.send_message_to_group(key, data, timestamp).await?;
            }
        }
        Ok(())
    }

    /// Send a message to the channel and add it to the channel.
    pub async fn send_message(&self, msg: Message) -> Result<(), crate::ApplicationError> {
        msg.mark_as_read();
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

    pub async fn participant_by_uuid(&self, uuid: Uuid) -> Contact {
        let found = self.participants().into_iter().find(|c| c.uuid() == uuid);
        if let Some(found) = found {
            return found;
        }
        let new =
            Contact::from_service_address(&ServiceId::Aci(uuid.into()), &self.manager()).await;
        self.imp().participants.borrow_mut().push(new.clone());
        new
    }

    async fn initialize_participants(&self) {
        let manager = self.manager();
        // Make sure to not hold reference accross await.
        let members = { self.group().as_ref().map(|g| g.members.clone()) };
        if let Some(members) = members {
            let mut participants = Vec::with_capacity(members.len());
            // XXX: Parallelize?
            for p in &members {
                let address = ServiceId::Aci(p.aci);
                let contact = Contact::from_service_address(&address, &manager).await;
                participants.push(contact);
            }
            for p in &participants {
                p.set_channel(Some(self));
                p.update_profile_name_and_avatar().await;
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

    pub async fn initialize_avatar(&self) {
        if let Some(context) = self.group_context() {
            let Some(avatar) = self
                .manager()
                .retrieve_group_avatar(context)
                .await
                .ok()
                .flatten()
                .and_then(|b| Texture::from_bytes(&Bytes::from_owned(b)).ok())
            else {
                log::debug!("Failed to fetch group avatar; it may not have a profile picture set",);
                return;
            };

            self.imp().group_avatar.replace(Some(avatar.into()));
        } else if let Some(contact) = self.contact() {
            contact.update_profile_name_and_avatar().await;
        }
        self.notify("avatar");
    }

    /// Cap the description to a single line.
    pub fn single_line_description(&self) -> Option<String> {
        Utility::single_line(self.description())
    }

    pub fn add_user_typing(&self, notification: TypingNotification) {
        let _ = self
            .imp()
            .typing
            .borrow_mut()
            .insert(notification.sender.uuid(), notification.clone());
        gspawn!(clone!(
            #[weak(rename_to = s)]
            self,
            #[strong]
            notification,
            async move {
                glib::timeout_future_seconds(TYPING_NOTIFICATION_DURATION_SECONDS).await;

                let mut typing = s.imp().typing.borrow_mut();
                if typing.get(&notification.sender.uuid()).map(|t| t.timestamp)
                    == Some(notification.timestamp)
                {
                    typing.remove(&notification.sender.uuid());
                    drop(typing);
                    s.notify("is-typing");
                    s.notify("typing-label");
                }
            }
        ));
        self.notify("is-typing");
        self.notify("typing-label");
    }

    pub fn remove_user_typing(&self, contact: Contact) {
        let mut typing = self.imp().typing.borrow_mut();
        typing.remove(&contact.uuid());
        drop(typing);
        self.notify("is-typing");
        self.notify("typing-label");
    }

    /// Split up the name into first names (if available) and last name.
    /// For groups, everything will be "last name".
    pub fn name_parts(&self) -> (Option<String>, String) {
        if self.is_self() {
            (None, gettextrs::gettext("Note to self"))
        } else if let Some(group) = &*self.group() {
            (None, group.title.clone())
        } else if let Some(contact) = self.contact() {
            contact.name_parts()
        } else {
            panic!("Contact is neither group nor contact; impossible.")
        }
    }

    pub fn last_name(&self) -> String {
        self.name_parts().1
    }

    /// The name which defines how channels are sorted. Depends on the settings.
    pub fn sort_name(&self) -> String {
        if self.manager().settings().string("sort-contacts-by") == "surname" {
            self.last_name()
        } else {
            self.title()
        }
    }

    /// Whether the channel is currently visible in the UI.
    pub fn set_active(&self, active: bool) {
        self.set_property("is-active", active);
    }

    fn first_unread_message(&self) -> Option<DisplayMessage> {
        self.imp()
            .timeline
            .borrow()
            .iter_backwards()
            .filter(|i| i.is::<DisplayMessage>())
            .map_while(|m| {
                let message = m.dynamic_cast::<DisplayMessage>().unwrap();
                // We can stop at first read message
                if !message.property::<bool>("read") {
                    return Some(message);
                }
                None
            })
            .last()
    }

    /// Mark all messages as read.
    pub fn mark_as_read(&self) -> Vec<Message> {
        if let Some(first_unread) = self.first_unread_message() {
            first_unread.flash_requires_attention();
        }

        self.imp()
            .timeline
            .borrow()
            .iter_backwards()
            .filter(|i| i.is::<DisplayMessage>() || i.is::<ReactionMessage>())
            .map_while(|m| {
                let message = m.dynamic_cast::<Message>().unwrap();
                // We can stop at first read message
                if message.mark_as_read() {
                    return Some(message);
                }
                None
            })
            .collect()
    }

    pub fn notification_variant(&self) -> (u8, Vec<u8>) {
        let thread = self.thread();
        match thread {
            Thread::Contact(u) => (0, u.service_id_binary()),
            Thread::Group(g) => (1, g.into()),
        }
    }

    pub fn from_notification_variant((kind, data): (u8, Vec<u8>)) -> Thread {
        match kind {
            0 => Thread::Contact(
                ServiceId::parse_from_service_id_binary(&data)
                    .expect("Notification variant for contact to be a service ID"),
            ),
            1 => Thread::Group(
                data.try_into()
                    .expect("Notification variant for groups to have the correct number of bytes"),
            ),
            _ => unreachable!("Unexpected notification variant type"),
        }
    }
}

mod imp {
    use super::TypingNotification;
    use crate::backend::{
        Contact, Manager,
        message::{DisplayMessage, ReactionMessage, TextMessage},
        timeline::Timeline,
    };
    use crate::prelude::*;

    use std::{
        collections::{BTreeSet, HashMap},
        marker::PhantomData,
    };

    use gdk::Paintable;

    use libsignal_service::proto::GroupContextV2;
    use presage::model::groups::Group;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Channel)]
    pub struct Channel {
        pub(super) contact: RefCell<Option<Contact>>,
        pub(super) group: RefCell<Option<Group>>,
        pub(super) group_context: RefCell<Option<GroupContextV2>>,

        pub(super) participants: RefCell<Vec<Contact>>,

        pub(super) pending_reactions: RefCell<HashMap<u64, Vec<ReactionMessage>>>,
        pub(super) typing: RefCell<HashMap<Uuid, TypingNotification>>,

        #[property(name = "avatar", get = Self::avatar)]
        pub(super) group_avatar: RefCell<Option<Paintable>>,
        #[property(get)]
        pub(super) timeline: RefCell<Timeline>,

        #[property(get, set)]
        pub(super) draft: RefCell<String>,
        #[property(get, set)]
        pub(super) is_active: RefCell<bool>,

        #[property(get = Self::last_message)]
        pub(super) last_message: PhantomData<Option<DisplayMessage>>,
        #[property(get = Self::title)]
        pub(super) title: PhantomData<String>,
        #[property(get = Self::is_contact)]
        pub(super) is_contact: PhantomData<bool>,
        #[property(get = Self::is_self)]
        pub(super) is_self: PhantomData<bool>,
        /// In seconds. A value of 0 means messages don't disappear.
        #[property(get = Self::disappearing_messages_timer)]
        pub(super) disappearing_messages_timer: PhantomData<u32>,
        #[property(get = Self::phone_number)]
        pub(super) phone_number: PhantomData<Option<String>>,
        #[property(get = Self::description)]
        pub(super) description: PhantomData<Option<String>>,
        #[property(get = Self::is_typing)]
        pub(super) is_typing: PhantomData<bool>,
        #[property(get = Self::typing_label)]
        pub(super) typing_label: PhantomData<String>,

        #[property(get, set, construct_only, type = Manager)]
        pub(super) manager: RefCell<Option<Manager>>,
    }

    impl Channel {
        fn avatar(&self) -> Option<Paintable> {
            if let Some(contact) = self.contact.borrow().as_ref() {
                contact.property("avatar")
            } else {
                self.group_avatar.borrow().clone()
            }
        }

        fn last_message(&self) -> Option<DisplayMessage> {
            self.timeline
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
        }

        fn title(&self) -> String {
            if let Some(group) = self.group.borrow().as_ref() {
                group.title.clone()
            } else if let Some(contact) = self.contact.borrow().as_ref() {
                if contact.property::<bool>("is-self") {
                    gettextrs::gettext("Note to self")
                } else {
                    contact.title()
                }
            } else {
                "".to_string()
            }
        }

        fn is_contact(&self) -> bool {
            self.contact.borrow().as_ref().is_some()
        }

        fn is_self(&self) -> bool {
            self.contact.borrow().as_ref().is_some_and(|c| c.is_self())
        }

        fn disappearing_messages_timer(&self) -> u32 {
            if let Some(group) = self.group.borrow().as_ref() {
                group
                    .disappearing_messages_timer
                    .as_ref()
                    .map(|t| t.duration)
                    .unwrap_or_default()
            } else if let Some(contact) = self.contact.borrow().as_ref() {
                contact.expire_timer()
            } else {
                0
            }
        }

        fn phone_number(&self) -> Option<String> {
            self.contact
                .borrow()
                .as_ref()
                .and_then(|c| c.phone_number())
        }

        fn description(&self) -> Option<String> {
            if let Some(contact) = self.contact.borrow().as_ref() {
                contact.description()
            } else {
                self.group
                    .borrow()
                    .as_ref()
                    .and_then(|g| g.description.clone())
            }
        }

        fn is_typing(&self) -> bool {
            !self.typing.borrow().is_empty()
        }

        fn typing_label(&self) -> String {
            if self.is_self() {
                gettextrs::gettext("is typing")
            } else {
                let contacts: BTreeSet<String> = self
                    .typing
                    .borrow()
                    .values()
                    .map(|n| n.sender.title())
                    .collect();
                let num_typing = contacts.len();
                let joined = contacts.into_iter().collect::<Vec<_>>().join(", ");
                gettextrs::ngettext("{} is typing", "{} are typing", num_typing as u32)
                    .replace("{}", &joined)
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Channel {
        const NAME: &'static str = "FlChannel";
        type Type = super::Channel;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Channel {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![
                    Signal::builder("message")
                        .param_types([DisplayMessage::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
}
