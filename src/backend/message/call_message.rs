use crate::prelude::*;

use crate::backend::timeline::TimelineItem;

use libsignal_service::content::CallMessage as PreCallMessage;

use crate::backend::{Channel, Contact};

use super::{DisplayMessage, Manager, Message};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, Default)]
#[repr(u32)]
#[enum_type(name = "FlCallMessageType")]
pub enum CallMessageType {
    #[default]
    Offer,
    Answer,
    Hangup,
    Busy,
}

impl TryFrom<&PreCallMessage> for CallMessageType {
    type Error = ();
    fn try_from(p: &PreCallMessage) -> Result<Self, Self::Error> {
        if p.offer.is_some() {
            Ok(CallMessageType::Offer)
        } else if p.hangup.is_some() {
            Ok(CallMessageType::Hangup)
        } else if p.answer.is_some() {
            Ok(CallMessageType::Answer)
        } else if p.busy.is_some() {
            Ok(CallMessageType::Busy)
        } else {
            Err(())
        }
    }
}

gtk::glib::wrapper! {
    /// A CallMessage represents a call event of some type [CallMessageType].
    pub struct CallMessage(ObjectSubclass<imp::CallMessage>) @extends Message, DisplayMessage, TimelineItem;
}

impl CallMessage {
    pub fn from_call(
        sender: &Contact,
        channel: &Channel,
        timestamp: u64,
        manager: &Manager,
        call: PreCallMessage,
    ) -> Option<Self> {
        let call_type = CallMessageType::try_from(&call).ok()?;
        let s: Self = Object::builder::<Self>()
            .property("sender", sender)
            .property("channel", channel)
            .property("timestamp", timestamp)
            .property("manager", manager)
            .property("call-type", call_type)
            .build();
        s.imp().call.swap(&RefCell::new(Some(call)));
        Some(s)
    }
}

mod imp {
    use super::*;

    use crate::backend::Message;
    use crate::backend::message::{
        DisplayMessage, MessageImpl, display_message::DisplayMessageImpl,
    };
    use crate::backend::timeline::{TimelineItem, TimelineItemImpl};

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::CallMessage)]
    pub struct CallMessage {
        pub(super) call: RefCell<Option<PreCallMessage>>,

        #[property(get, set, construct_only, default = CallMessageType::default(), builder(CallMessageType::default()))]
        pub(super) call_type: RefCell<CallMessageType>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CallMessage {
        const NAME: &'static str = "FlCallMessage";
        type Type = super::CallMessage;
        type ParentType = DisplayMessage;
    }

    impl DisplayMessageImpl for CallMessage {
        fn textual_description(&self, obj: &super::CallMessage) -> Option<String> {
            let sender: Contact = obj.property("sender");

            match (obj.call_type(), sender.is_self()) {
                (CallMessageType::Offer, false) => Some(gettextrs::gettext("Incoming call")),
                (CallMessageType::Offer, true) => Some(gettextrs::gettext("Outgoing call")),
                (CallMessageType::Answer, _) => Some(gettextrs::gettext("Call started")),
                (CallMessageType::Hangup, _) => Some(gettextrs::gettext("Call ended")),
                (CallMessageType::Busy, false) => Some(gettextrs::gettext("Call declined")),
                (CallMessageType::Busy, true) => Some(gettextrs::gettext("Unanswered call")),
            }
        }
    }

    impl TimelineItemImpl for CallMessage {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_header(upcast, previous);
        }

        fn update_show_timestamp(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_timestamp(upcast, previous);
        }
    }

    impl MessageImpl for CallMessage {}

    #[glib::derived_properties]
    impl ObjectImpl for CallMessage {}
}
