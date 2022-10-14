use gdk_pixbuf::Pixbuf;
use gtk::{glib, prelude::*, subclass::prelude::*};

use super::{Message, MessageExt, MessageImpl};

glib::wrapper! {
    pub struct DisplayMessage(ObjectSubclass<imp::DisplayMessage>) @extends Message;
}

impl DisplayMessage {
    pub fn send_notification(&self) {
        let sender = self.sender();
        let body = self.textual_description();
        if sender.is_self() || body.is_none() || body.as_ref().unwrap().is_empty() {
            // Skip notifications for messages sent from self or empty messages.
            return;
        }

        let notification = gio::Notification::new(&sender.title());
        notification.set_body(body.as_deref());
        let icon = Pixbuf::from_resource("/icon.png").expect("Flare to have an application icon");
        notification.set_icon(&icon);
        self.manager().send_notification(&notification);
    }
}

pub trait DisplayMessageExt: 'static {
    fn textual_description(&self) -> Option<String>;

    fn send_notification(&self)
    where
        Self: IsA<DisplayMessage>,
    {
        self.upcast_ref::<DisplayMessage>().send_notification()
    }
}

impl<O: IsA<DisplayMessage>> DisplayMessageExt for O {
    fn textual_description(&self) -> Option<String> {
        imp::display_message_textual_description(self.upcast_ref())
    }
}

pub trait DisplayMessageImpl: ObjectImpl {
    fn textual_description(&self, _obj: &Self::Type) -> Option<String> {
        None
    }
}

unsafe impl<T> IsSubclassable<T> for DisplayMessage
where
    T: DisplayMessageImpl + MessageImpl,
    T::Type: IsA<DisplayMessage>,
{
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class.upcast_ref_mut());

        let klass = class.as_mut();

        klass.textual_description = textual_description_trampoline::<T>;
    }
}

fn textual_description_trampoline<T>(this: &DisplayMessage) -> Option<String>
where
    T: ObjectSubclass + DisplayMessageImpl,
    T::Type: IsA<DisplayMessage>,
{
    let this = this.downcast_ref::<T::Type>().unwrap();
    this.imp().textual_description(this)
}

mod imp {
    use glib::{
        once_cell::sync::Lazy,
        subclass::types::{ClassStruct, ObjectSubclass},
        ParamFlags, ParamSpec, ParamSpecString,
    };

    use crate::backend::{message::MessageImpl, Message};

    use super::*;

    #[repr(C)]
    pub struct DisplayMessageClass {
        pub parent_class: glib::object::ObjectClass,
        pub textual_description: fn(&super::DisplayMessage) -> Option<String>,
    }

    unsafe impl ClassStruct for DisplayMessageClass {
        type Type = DisplayMessage;
    }

    pub(super) fn display_message_textual_description(
        this: &super::DisplayMessage,
    ) -> Option<String> {
        let klass = this.class();
        (klass.as_ref().textual_description)(this)
    }

    #[derive(Debug, Default)]
    pub struct DisplayMessage {}

    #[glib::object_subclass]
    impl ObjectSubclass for DisplayMessage {
        const NAME: &'static str = "FlDisplayMessage";
        type Type = super::DisplayMessage;
        type ParentType = Message;
        type Class = DisplayMessageClass;
    }

    impl ObjectImpl for DisplayMessage {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecString::new(
                    "textual-description",
                    "textual-description",
                    "textual-description",
                    None,
                    ParamFlags::READABLE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            _value: &glib::Value,
            _pspec: &glib::ParamSpec,
        ) {
            unimplemented!()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "textual-description" => obj.textual_description().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl MessageImpl for DisplayMessage {}
}
