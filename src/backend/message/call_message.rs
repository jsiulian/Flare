use crate::backend::timeline::TimelineItem;
use std::cell::RefCell;

use gdk::prelude::ObjectExt;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::Object;
use gtk::{gdk, gio, glib};
use presage::prelude::content::CallMessage as PreCallMessage;

use crate::backend::{Channel, Contact};

use super::{DisplayMessage, Manager, Message};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "FlCallMessageType")]
pub enum CallMessageType {
    Offer,
    Answer,
    Hangup,
    Busy,
}

impl Default for CallMessageType {
    fn default() -> Self {
        Self::Offer
    }
}

impl TryFrom<&PreCallMessage> for CallMessageType {
    type Error = ();
    fn try_from(p: &PreCallMessage) -> Result<Self, Self::Error> {
        if p.offer.is_some() {
            Ok(CallMessageType::Offer)
        } else if p.hangup.is_some() || p.legacy_hangup.is_some() {
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
            .property("timestamp", &timestamp)
            .property("manager", manager)
            .property("call-type", &call_type)
            .build();
        s.imp().call.swap(&RefCell::new(Some(call)));
        Some(s)
    }

    pub fn call_type(&self) -> CallMessageType {
        self.property("call-type")
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass, ObjectSubclassIsExt};
    use gdk::{
        gdk_pixbuf::{
            glib::{once_cell::sync::Lazy, ParamSpec, Value},
            prelude::ToValue,
        },
        prelude::ParamSpecBuilderExt,
    };
    use glib::ParamSpecEnum;
    use gtk::{glib, prelude::Cast};
    use presage::prelude::content::CallMessage as PreCallMessage;
    use std::cell::RefCell;

    use crate::backend::message::{
        display_message::DisplayMessageImpl, DisplayMessage, MessageImpl,
    };
    use crate::backend::timeline::{TimelineItem, TimelineItemImpl};
    use crate::backend::Message;

    use super::CallMessageType;

    #[derive(Default)]
    pub struct CallMessage {
        pub(super) call: RefCell<Option<PreCallMessage>>,

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
            match obj.call_type() {
                CallMessageType::Offer => Some(gettextrs::gettext("Started calling.")),
                CallMessageType::Answer => Some(gettextrs::gettext("Answered a call.")),
                CallMessageType::Hangup => Some(gettextrs::gettext("Hung up.")),
                CallMessageType::Busy => Some(gettextrs::gettext("Is busy.")),
            }
        }
    }

    impl TimelineItemImpl for CallMessage {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_header(upcast, previous);
        }
    }

    impl MessageImpl for CallMessage {}

    impl ObjectImpl for CallMessage {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecEnum::builder("call-type")
                    .default_value(CallMessageType::default())
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "call-type" => self.call_type.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "call-type" => {
                    let obj = value.get::<CallMessageType>().expect(
                        "Property `call-type` of `CallMessage` has to be of type `CallMessageType`",
                    );

                    self.call_type.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
