use adw::subclass::prelude::*;
use gdk::glib::Object;
use gtk::{gdk, glib, prelude::*, CompositeTemplate};

use crate::backend::message::TextMessage;

glib::wrapper! {
    pub struct TimeDivider(ObjectSubclass<imp::TimeDivider>)
        @extends gtk::Box, gtk::Widget, @implements gtk::Accessible;
}

impl Default for TimeDivider {
    fn default() -> Self {
        Object::builder::<Self>().build()
    }
}

mod imp {
    use gdk::glib::subclass::Signal;
    use glib::subclass::InitializingObject;
    use gtk::subclass::box_::BoxImpl;
    use once_cell::sync::Lazy;

    use crate::{
        backend::timeline::{TimelineItem, TimelineItemExt},
        gui::utility::Utility,
    };

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/ui/components/time_divider.ui")]
    pub struct TimeDivider {
        #[template_child]
        label_date: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimeDivider {
        const NAME: &'static str = "TimeDivider";
        type Type = super::TimeDivider;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TimeDivider {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<TimelineItem>("item")
                    .write_only()
                    .build()]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "item" => {
                    let v = value
                        .get::<Option<TimelineItem>>()
                        .expect("TimeDivider to only get TimelineItem");

                    let formatted = v
                        .and_then(|v| v.datetime())
                        .and_then(|d| Utility::format_date_human(&d));
                    self.label_date.set_text(&formatted.unwrap_or_default());
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

    impl WidgetImpl for TimeDivider {}
    impl BoxImpl for TimeDivider {}
}
