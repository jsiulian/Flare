use adw::subclass::prelude::*;
use gdk::glib::Object;
use gtk::{gdk, glib, prelude::*, CompositeTemplate};

glib::wrapper! {
    pub struct LetterDivider(ObjectSubclass<imp::LetterDivider>)
        @extends gtk::Box, gtk::Widget, @implements gtk::Accessible;
}

impl Default for LetterDivider {
    fn default() -> Self {
        Object::builder::<Self>().build()
    }
}

mod imp {
    use glib::subclass::InitializingObject;
    use gtk::subclass::box_::BoxImpl;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/ui/components/letter_divider.ui")]
    pub struct LetterDivider {
        #[template_child]
        label_char: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LetterDivider {
        const NAME: &'static str = "LetterDivider";
        type Type = super::LetterDivider;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LetterDivider {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecString::builder("string")
                    .write_only()
                    .build()]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "string" => {
                    let v = value
                        .get::<Option<String>>()
                        .expect("LetterDivider to only get String");

                    let formatted = v.and_then(|v| v.chars().next()).map(|c| c.to_uppercase().to_string());
                    self.label_char.set_text(&formatted.unwrap_or_default());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, _pspec: &glib::ParamSpec) -> glib::Value {
            unimplemented!();
        }
    }

    impl WidgetImpl for LetterDivider {}
    impl BoxImpl for LetterDivider {}
}
