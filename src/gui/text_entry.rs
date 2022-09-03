use gdk::subclass::prelude::ObjectSubclassIsExt;
use gtk::traits::TextBufferExt;

gtk::glib::wrapper! {
    pub struct TextEntry(ObjectSubclass<imp::TextEntry>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl TextEntry {
    pub fn text(&self) -> String {
        let obj = self.imp();
        let buffer = &obj.buffer;
        let (start_iter, end_iter) = buffer.bounds();
        buffer.text(&start_iter, &end_iter, true).to_string()
    }
    pub fn clear(&self) {
        let obj = self.imp();
        let buffer = &obj.buffer;
        buffer.set_text("");
    }
}

pub mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::subclass::Signal;
    use gdk_pixbuf::glib::{
        self, once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, Value,
    };
    use gdk_pixbuf::prelude::ObjectExt;
    use gtk::subclass::widget::{CompositeTemplate, WidgetClassSubclassExt};
    use gtk::traits::WidgetExt;
    use gtk::{
        prelude::{InitializingWidgetExt, StaticType},
        subclass::{prelude::BoxImpl, widget::WidgetImpl},
        CompositeTemplate,
    };
    use gtk::{Inhibit, TextView};
    use gtk::{TemplateChild, TextBuffer};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/text_entry.ui")]
    pub struct TextEntry {
        #[template_child]
        pub(super) view: TemplateChild<TextView>,
        #[template_child]
        pub(super) buffer: TemplateChild<TextBuffer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextEntry {
        const NAME: &'static str = "FlTextEntry";
        type Type = super::TextEntry;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TextEntry {
        fn constructed(&self, obj: &Self::Type) {
            let key_events = gtk::EventControllerKey::new();
            self.view.add_controller(&key_events);
            key_events
                .connect_key_pressed(clone!(@weak obj => @default-return Inhibit(false), move |_, key, _, modifier| {
                if !modifier.contains(gdk::ModifierType::SHIFT_MASK) && (key == gdk::Key::Return || key == gdk::Key::KP_Enter) {
                    obj.emit_by_name::<()>("activate", &[]);
                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            }));
        }
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| vec![]);
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, _value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("activate", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for TextEntry {}
    impl BoxImpl for TextEntry {}
}
