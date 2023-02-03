use std::cell::RefCell;

use gtk::{gio, glib};
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use presage::prelude::content::Reaction;
use presage::prelude::*;

use crate::backend::{Channel, Contact};

use super::{Manager, Message};

gtk::glib::wrapper! {
    pub struct ReactionMessage(ObjectSubclass<imp::ReactionMessage>) @extends Message;
}

impl ReactionMessage {
    pub fn from_reaction(
        sender: &Contact,
        channel: &Channel,
        timestamp: u64,
        manager: &Manager,
        reaction: Reaction,
    ) -> Self {
        let s: Self = Object::new::<Self>(&[
            ("sender", sender),
            ("channel", channel),
            ("sent", &timestamp),
            ("manager", manager),
        ]);
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
        Uuid::parse_str(self.reaction().target_author_uuid()).expect("`Reaction` Uuid to be valid")
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gtk::glib;
    use presage::prelude::content::Reaction;
    use std::cell::RefCell;

    use crate::backend::{message::MessageImpl, Message};

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

    impl MessageImpl for ReactionMessage {}

    impl ObjectImpl for ReactionMessage {}
}
