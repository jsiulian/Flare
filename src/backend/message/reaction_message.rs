use std::cell::RefCell;

use gdk::gdk_pixbuf::Pixbuf;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use gtk::{gio, glib};
use libsignal_service::{content::Reaction, prelude::Uuid};

use crate::backend::{timeline::TimelineItem, Channel, Contact};

use super::{Manager, Message, MessageExt};

use gettextrs::gettext;

gtk::glib::wrapper! {
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
        Uuid::parse_str(self.reaction().target_author_aci()).expect("`Reaction` Uuid to be valid")
    }

    pub fn send_notification(&self) {
        let sender = self.sender();
        let channel = self.channel();
        if sender.is_self() || self.target_uuid() != self.manager().uuid() {
            // Skip notifications for reaction messages sent from self.
            return;
        }
        let notification_title;
        let notification_body;
        if channel.group_context().is_some() {
            notification_title = channel.title();
            // Translators: When receiving a reaction message in a group, this will be the text to format the notification. Do not translate the text in {}.
            notification_body = gettext("{sender} reacted {emoji} to a message.")
                .replace("{sender}", &sender.title())
                .replace("{emoji}", &self.emoji());
        } else {
            notification_title = sender.title();
            // Translators: When receiving a reaction message in a 1-to-1 channel (the body, the sender will be in the notification title and is not included in the translation), this will be the text to format the notification. Do not translate the text in {}.
            notification_body = gettext("Reacted {emoji} to a message.")
                .replace("{sender}", &sender.title())
                .replace("{emoji}", &self.emoji());
        }
        let notification = gio::Notification::new(&notification_title);
        notification.set_body(Some(&notification_body));
        let icon = Pixbuf::from_resource("/icon.png").expect("Flare to have an application icon");
        notification.set_icon(&icon);

        let manager = self.manager();
        crate::gspawn!(async move {
            manager.send_notification(&notification).await;
        });
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gtk::glib;
    use libsignal_service::content::Reaction;
    use std::cell::RefCell;

    use crate::backend::{message::MessageImpl, timeline::TimelineItemImpl, Message};

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
