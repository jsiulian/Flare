use gdk_pixbuf::glib::Object;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl Attachment {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::new(&[("attachment", attachment)]).expect("Failed to create `Attachment`")
    }
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use crate::backend::Manager;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/attachment.ui")]
    pub struct Attachment {
        attachment: RefCell<Option<crate::backend::Attachment>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Attachment {
        const NAME: &'static str = "FlAttachment";
        type Type = super::Attachment;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Attachment {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "attachment",
                        "attachment",
                        "attachment",
                        crate::backend::Attachment::static_type(),
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "attachment" => self.attachment.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Attachment` has to be of type `Manager`");
                    self.manager.replace(man);
                }
                "attachment" => {
                    let att = value
                        .get::<Option<crate::backend::Attachment>>()
                        .expect("Property `attachment` of `Attachment` has to be of type `crate::backend::Attachment`");
                    self.attachment.replace(att);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Attachment {}
    impl BoxImpl for Attachment {}
}
