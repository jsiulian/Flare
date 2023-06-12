use glib::{prelude::IsA, Object};
use gtk::glib;
use libadwaita::traits::MessageDialogExt;

use crate::ApplicationError;

const REPORT: &str = "report";

glib::wrapper! {
    pub struct ErrorDialog(ObjectSubclass<imp::ErrorDialog>)
        @extends libadwaita::MessageDialog, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ErrorDialog {
    pub fn new(error: ApplicationError, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing ErrorDialog");
        log::error!("ErrorDialog displaying error: {}", error);
        log::trace!("ErrorDialog full error: {:#?}", error);
        let s: Self = Object::builder::<Self>()
            .property("body", &error.to_string())
            .property("secondary-error", &error.more_information())
            .property("should-report", error.should_report())
            .property("transient-for", parent)
            .build();
        s.set_response_enabled(REPORT, error.should_report());
        s
    }
}

pub mod imp {
    pub(crate) use std::cell::Cell;
    use std::cell::RefCell;

    use gdk::gio;
    use glib::{
        once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecBoolean,
        ParamSpecString, Value,
    };
    use gtk::{glib, UriLauncher};
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use libadwaita::subclass::prelude::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/error_dialog.ui")]
    pub struct ErrorDialog {
        secondary_error: RefCell<Option<String>>,
        should_report: Cell<bool>,
    }

    #[gtk::template_callbacks]
    impl ErrorDialog {
        #[template_callback]
        fn handle_response(&self, response: &str) {
            log::info!("Response: {}", response);
            if response == super::REPORT {
                let launcher = UriLauncher::new("https://gitlab.com/Schmiddiii/flare/-/issues");
                launcher.launch(
                    self.obj().transient_for().as_ref(),
                    None::<&gio::Cancellable>,
                    |_| {},
                );
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ErrorDialog {
        const NAME: &'static str = "FlErrorDialog";
        type Type = super::ErrorDialog;
        type ParentType = libadwaita::MessageDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ErrorDialog {
        fn constructed(&self) {
            log::trace!("Constructed ErrorDialog");
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("secondary-error")
                        .construct_only()
                        .build(),
                    ParamSpecBoolean::builder("should-report")
                        .construct_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "secondary-error" => self.secondary_error.borrow().as_ref().to_value(),
                "should-report" => self.should_report.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
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

    impl MessageDialogImpl for ErrorDialog {}
    impl WidgetImpl for ErrorDialog {}
    impl WindowImpl for ErrorDialog {}
    impl ApplicationWindowImpl for ErrorDialog {}
    impl AdwWindowImpl for ErrorDialog {}
    impl AdwApplicationWindowImpl for ErrorDialog {}
}
