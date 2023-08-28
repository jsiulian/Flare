use adw::subclass::prelude::*;
use gdk::glib::{clone, Object, SignalHandlerId};
use gtk::{gdk, glib, prelude::*, CompositeTemplate};

use crate::{
    backend::{
        message::{CallMessage, TextMessage},
        timeline::TimelineItem,
    },
    gui::{call_message_item::CallMessageItem, message_item::MessageItem},
};

glib::wrapper! {
    pub struct ItemRow(ObjectSubclass<imp::ItemRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Default for ItemRow {
    fn default() -> Self {
        Object::builder::<Self>().build()
    }
}

impl ItemRow {
    fn timeline_item_to_widget(&self, item: &TimelineItem) -> Option<gtk::Widget> {
        if let Some(message) = item.dynamic_cast_ref::<TextMessage>() {
            let widget = MessageItem::new(message);
            let handler = widget.connect_local(
                "reply",
                false,
                clone!(@weak self as s => @default-return None, move |args| {
                    let msg = args[1]
                        .get::<TextMessage>()
                        .expect("Type of signal `reply` of `MessageItem` to be `TextMessage`.");
                    s.emit_by_name::<()>("reply", &[&msg]);
                    None
                }),
            );
            self.set_handler(handler);
            Some(widget.dynamic_cast().unwrap())
        } else if let Some(message) = item.dynamic_cast_ref::<CallMessage>() {
            let widget = CallMessageItem::new(message);
            Some(widget.dynamic_cast().unwrap())
        } else {
            log::warn!("`ItemRow` was asked to display an unknown `TimelineItem`");
            None
        }
    }

    /// Set the pending handler of the ItemRow.
    /// At most one handler may be pending.
    fn set_handler(&self, handler: SignalHandlerId) {
        self.imp().handler.replace(Some(handler));
    }
}

mod imp {
    use adw::traits::BinExt;
    use gdk::glib::subclass::Signal;
    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;

    use crate::backend::timeline::TimelineItem;

    use super::*;

    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/ui/components/item_row.ui")]
    pub struct ItemRow {
        pub(super) handler: RefCell<Option<SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ItemRow {
        const NAME: &'static str = "ItemRow";
        type Type = super::ItemRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ItemRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<TimelineItem>("item")
                    .write_only()
                    .build()]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();
            match pspec.name() {
                "item" => {
                    let v = value
                        .get::<Option<TimelineItem>>()
                        .expect("ItemRow to only get TimelineItem");

                    if let Some(handler) = self.handler.take() {
                        if let Some(child) = obj.child() {
                            child.disconnect(handler);
                        } else {
                            log::warn!("A handler was set for an item row, but no child registered. This should not happen.");
                        }
                    }

                    let w = v.and_then(|v| obj.timeline_item_to_widget(&v));
                    obj.set_child(w.as_ref());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, _pspec: &glib::ParamSpec) -> glib::Value {
            unimplemented!();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("reply")
                    .param_types([TextMessage::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for ItemRow {}
    impl BinImpl for ItemRow {}
}
