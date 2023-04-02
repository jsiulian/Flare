use std::cell::RefCell;

use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use gtk::{gio, glib};
use libsignal_service::proto::data_message::Delete;

use crate::backend::{Channel, Contact};

use super::{Manager, Message};

gtk::glib::wrapper! {
    pub struct DeletionMessage(ObjectSubclass<imp::DeletionMessage>) @extends Message;
}

impl DeletionMessage {
    pub fn from_delete(
        sender: &Contact,
        channel: &Channel,
        timestamp: u64,
        manager: &Manager,
        deletion: Delete,
    ) -> Self {
        let s: Self = Object::builder::<Self>()
            .property("sender", sender)
            .property("channel", channel)
            .property("sent", &timestamp)
            .property("manager", manager)
            .build();
        s.imp().deletion.swap(&RefCell::new(Some(deletion)));
        s
    }

    pub fn deletion(&self) -> Delete {
        self.imp()
            .deletion
            .borrow()
            .clone()
            .expect("`DeletionMessage` to have a `Delete`")
    }

    pub fn target_timestamp(&self) -> u64 {
        self.deletion().target_sent_timestamp()
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gtk::glib;
    use libsignal_service::proto::data_message::Delete;
    use std::cell::RefCell;

    use crate::backend::{message::MessageImpl, Message};

    #[derive(Default)]
    pub struct DeletionMessage {
        pub(super) deletion: RefCell<Option<Delete>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeletionMessage {
        const NAME: &'static str = "FlDeletionMessage";
        type Type = super::DeletionMessage;
        type ParentType = Message;
    }

    impl MessageImpl for DeletionMessage {}

    impl ObjectImpl for DeletionMessage {}
}
