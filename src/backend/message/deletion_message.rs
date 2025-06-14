use crate::backend::timeline::TimelineItem;
use crate::backend::{Channel, Contact};
use crate::prelude::*;

use libsignal_service::proto::data_message::Delete;

use super::{Manager, Message};

gtk::glib::wrapper! {
    /// A deletion message is propagated to mark a message as deleted.
    pub struct DeletionMessage(ObjectSubclass<imp::DeletionMessage>) @extends Message, TimelineItem;
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
            .property("timestamp", timestamp)
            .property("manager", manager)
            .build();
        s.imp().deletion.swap(&RefCell::new(Some(deletion)));
        s
    }

    pub fn deletion(&self) -> Delete {
        self.imp()
            .deletion
            .borrow()
            .expect("`DeletionMessage` to have a `Delete`")
    }

    pub fn target_timestamp(&self) -> u64 {
        self.deletion().target_sent_timestamp()
    }
}

mod imp {
    use crate::backend::{Message, message::MessageImpl, timeline::TimelineItemImpl};
    use crate::prelude::*;

    use libsignal_service::proto::data_message::Delete;

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

    impl TimelineItemImpl for DeletionMessage {}
    impl MessageImpl for DeletionMessage {}
    impl ObjectImpl for DeletionMessage {}
}
