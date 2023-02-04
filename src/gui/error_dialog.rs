use gtk::glib;
use glib::{prelude::IsA, Object};

use crate::ApplicationError;

glib::wrapper! {
    pub struct ErrorDialog(ObjectSubclass<imp::ErrorDialog>)
        @extends gtk::Dialog, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ErrorDialog {
    pub fn new(error: ApplicationError, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing ErrorDialog");
        log::error!("ErrorDialog displaying error: {}", error);
        log::trace!("ErrorDialog full error: {:#?}", error);
        Object::new::<Self>(&[
            ("error", &error.to_string()),
            ("secondary-error", &error.more_information()),
            ("should-report", &error.should_report()),
            ("transient-for", &parent),
        ])
    }
}

pub mod imp {
    pub(crate) use std::cell::Cell;
    use std::cell::RefCell;

    use gtk::glib;
    use glib::{
        once_cell::sync::Lazy, subclass::InitializingObject, ParamFlags, ParamSpec,
        ParamSpecBoolean, ParamSpecString, Value,
    };
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use libadwaita::subclass::prelude::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/error_dialog.ui")]
    pub struct ErrorDialog {
        error: RefCell<Option<String>>,
        secondary_error: RefCell<Option<String>>,
        should_report: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ErrorDialog {
        const NAME: &'static str = "FlErrorDialog";
        type Type = super::ErrorDialog;
        type ParentType = gtk::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ErrorDialog {
        fn constructed(&self) {
            log::trace!("Constructed ErrorDialog");
            self.parent_constructed();
            self.instance().connect_response(|dialog, _| dialog.close());
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::new(
                        "error",
                        "error",
                        "error",
                        None,
                        ParamFlags::READWRITE | ParamFlags::CONSTRUCT_ONLY,
                    ),
                    ParamSpecString::new(
                        "secondary-error",
                        "secondary-error",
                        "secondary-error",
                        None,
                        ParamFlags::READWRITE | ParamFlags::CONSTRUCT_ONLY,
                    ),
                    ParamSpecBoolean::new(
                        "should-report",
                        "should-report",
                        "should-report",
                        false,
                        ParamFlags::READWRITE | ParamFlags::CONSTRUCT_ONLY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "error" => self.error.borrow().as_ref().to_value(),
                "secondary-error" => self.secondary_error.borrow().as_ref().to_value(),
                "should-report" => self.should_report.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "error" => {
                    let e = value
                        .get::<Option<String>>()
                        .expect("Property `error` of `ErrorDialog` has to be of type `String`");
                    self.error.replace(e);
                }
                "secondary-error" => {
                    let e = value.get::<Option<String>>().expect(
                        "Property `secondary-error` of `ErrorDialog` has to be of type `String`",
                    );
                    self.secondary_error.replace(e);
                }
                "should-report" => {
                    let r = value.get::<bool>().expect(
                        "Property `should-report` of `ErrorDialog` has to be of type `bool`",
                    );
                    self.should_report.replace(r);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl DialogImpl for ErrorDialog {}
    impl WidgetImpl for ErrorDialog {}
    impl WindowImpl for ErrorDialog {}
    impl ApplicationWindowImpl for ErrorDialog {}
    impl AdwWindowImpl for ErrorDialog {}
    impl AdwApplicationWindowImpl for ErrorDialog {}
}
