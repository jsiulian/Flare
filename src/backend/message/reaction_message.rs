use crate::backend::{Channel, Contact, timeline::TimelineItem};
use crate::prelude::*;

use gdk_pixbuf::Pixbuf;
use gettextrs::gettext;

use libsignal_service::{content::Reaction, prelude::Uuid};

use super::{Manager, Message, MessageExt};

gtk::glib::wrapper! {
    /// A ReactionMessage is propagated to add or remove a reaction from a [Message].
    pub struct ReactionMessage(ObjectSubclass<imp::ReactionMessage>) @extends Message, TimelineItem;
}

impl ReactionMessage {
    pub fn from_reaction(
        sender: &Contact,
        channel: &Channel,
        timestamp: u64,
        manager: &Manager,
        reaction: Reaction,
    ) -> Self {
        let s: Self = Object::builder::<Self>()
            .property("sender", sender)
            .property("channel", channel)
            .property("timestamp", timestamp)
            .property("manager", manager)
            .property("read", channel.property::<bool>("is-active"))
            .build();
        s.imp().reaction.swap(&RefCell::new(Some(reaction)));
        s
    }

    pub fn reaction(&self) -> Reaction {
        self.imp()
            .reaction
            .borrow()
            .clone()
            .expect("`ReactionMessage` to have a `Reaction`")
    }

    pub fn emoji(&self) -> String {
        self.reaction().emoji.unwrap_or_default()
    }

    pub fn target_timestamp(&self) -> u64 {
        self.reaction().target_sent_timestamp()
    }

    pub fn target_uuid(&self) -> Uuid {
        let reaction = self.reaction();
        reaction
            .target_author_aci_binary
            .as_ref()
            .and_then(|t| t.clone().try_into().ok())
            .map(Uuid::from_bytes)
            .or(Uuid::parse_str(reaction.target_author_aci()).ok())
            .expect("Reaction to have a valid target UUID")
    }

    /// Send a notification for the reaction.
    ///
    /// This is skipped if:
    /// - The reaction is sent from self.
    /// - The reaction targets a message from someone else.
    pub fn send_notification(&self) {
        let sender = self.sender();
        let channel = self.channel();
        if sender.is_self() || self.target_uuid() != self.manager().uuid() {
            // Skip notifications for reaction messages sent from self.
            return;
        }

        let (notification_title, notification_body) = if channel.group_context().is_some() {
            // Translators: When receiving a reaction message in a group, this will be the text to format the notification. Do not translate the text in {}.
            (
                channel.title(),
                gettext("{sender} reacted {emoji} to a message.")
                    .replace("{sender}", &sender.title())
                    .replace("{emoji}", &self.emoji()),
            )
        } else {
            // Translators: When receiving a reaction message in a 1-to-1 channel (the body, the sender will be in the notification title and is not included in the translation), this will be the text to format the notification. Do not translate the text in {}.
            (
                sender.title(),
                gettext("Reacted {emoji} to a message.")
                    .replace("{sender}", &sender.title())
                    .replace("{emoji}", &self.emoji()),
            )
        };

        let icon = Pixbuf::from_resource("/icon.svg").expect("Flare to have an application icon");

        let notification = gio::Notification::new(&notification_title);
        notification.set_body(Some(&notification_body));
        notification.set_icon(&icon);
        notification.set_category(Some("im.received"));
        notification.set_default_action_and_target_value(
            "app.notification-clicked",
            Some(&channel.notification_variant().to_variant()),
        );

        let manager = self.manager();
        let uid = self.uid();
        manager.send_notification(uid, &notification);
    }
}

mod imp {
    use crate::prelude::*;
    use libsignal_service::content::Reaction;

    use crate::backend::{Message, message::MessageImpl, timeline::TimelineItemImpl};

    #[derive(Default)]
    pub struct ReactionMessage {
        pub(super) reaction: RefCell<Option<Reaction>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReactionMessage {
        const NAME: &'static str = "FlReactionMessage";
        type Type = super::ReactionMessage;
        type ParentType = Message;
    }

    impl TimelineItemImpl for ReactionMessage {}
    impl MessageImpl for ReactionMessage {}
    impl ObjectImpl for ReactionMessage {}
}
