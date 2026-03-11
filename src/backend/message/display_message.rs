use crate::prelude::*;

use crate::backend::timeline::{TimelineItem, TimelineItemImpl};

use gdk_pixbuf::Pixbuf;
use gio::Notification;

use super::{Message, MessageExt, MessageImpl};

glib::wrapper! {
    /// A DisplayMessage is a message that should be shown to the user within the UI of Flare and which may also be notified about.
    pub struct DisplayMessage(ObjectSubclass<imp::DisplayMessage>) @extends Message, TimelineItem;
}

impl DisplayMessage {
    /// Send a notification if needed.
    ///
    /// Sending a notification is skipped if:
    /// - The message is from self, or
    /// - The message has no textual description
    pub fn send_notification(&self) {
        let sender = self.sender();
        let channel = self.channel();
        let body = self.textual_description();
        if sender.is_self() || body.is_none() || body.as_ref().unwrap().is_empty() {
            // Skip notifications for messages sent from self or empty messages.
            return;
        }
        let (notification_title, notification_body) = if channel.group_context().is_some() {
            (
                channel.title(),
                format!("{}: {}", sender.title(), body.unwrap_or_default()),
            )
        } else {
            (sender.title(), body.unwrap_or_default())
        };

        let icon = Pixbuf::from_resource("/icon.svg").expect("Flare to have an application icon");

        let notification = Notification::new(&notification_title);
        notification.set_body(Some(&notification_body));
        notification.set_icon(&icon);
        notification.set_category(Some("im.received"));
        notification.set_default_action_and_target_value(
            "app.notification-clicked",
            Some(&channel.notification_variant().to_variant()),
        );

        let manager = self.manager();
        let uid = self.uid();
        crate::gspawn!(async move {
            manager.send_notification(uid, &notification).await;
        });
    }
}

pub trait DisplayMessageExt: 'static {
    fn textual_description(&self) -> Option<String>;
    fn flash_requires_attention(&self);
}

impl<O: IsA<DisplayMessage>> DisplayMessageExt for O {
    fn textual_description(&self) -> Option<String> {
        imp::display_message_textual_description(self.upcast_ref())
    }

    fn flash_requires_attention(&self) {
        self.set_property("requires-attention", true);
        gspawn!(clone!(
            #[strong(rename_to = s)]
            self,
            async move {
                glib::timeout_future_seconds(2).await;
                s.set_property("requires-attention", false);
            }
        ));
    }
}

pub trait DisplayMessageImpl: ObjectImpl {
    fn textual_description(&self, _obj: &Self::Type) -> Option<String> {
        None
    }
}

unsafe impl<T> IsSubclassable<T> for DisplayMessage
where
    T: DisplayMessageImpl + MessageImpl + TimelineItemImpl,
    T::Type: IsA<DisplayMessage> + IsA<Message> + IsA<TimelineItem>,
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
    use crate::prelude::*;

    use gio::glib::ParamSpecBoolean;
    use glib::{
        ParamSpec, ParamSpecString,
        subclass::types::{ClassStruct, ObjectSubclass},
    };

    use super::DisplayMessageExt;
    use crate::backend::timeline::{TimelineItem, TimelineItemImpl};
    use crate::backend::{Message, message::MessageImpl};

    #[repr(C)]
    pub struct DisplayMessageClass {
        pub parent_class: glib::Class<Message>,
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
    pub struct DisplayMessage {
        pub(super) requires_attention: Cell<bool>,
    }

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
                vec![
                    ParamSpecString::builder("textual-description")
                        .read_only()
                        .build(),
                    ParamSpecBoolean::builder("requires-attention").build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "requires-attention" => self.requires_attention.set(
                    value
                        .get()
                        .expect("requires-attention parameter to be boolean"),
                ),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "textual-description" => self.obj().textual_description().to_value(),
                "requires-attention" => self.requires_attention.get().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl TimelineItemImpl for DisplayMessage {
        fn update_show_header(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_header(upcast, previous);
        }
        fn update_show_timestamp(&self, obj: &Self::Type, previous: Option<&TimelineItem>) {
            let upcast = obj.upcast_ref::<Message>();
            upcast.imp().update_show_timestamp(upcast, previous);
        }
    }

    impl MessageImpl for DisplayMessage {}
}
