use crate::prelude::*;

use crate::ApplicationError;

const REPORT: &str = "report";

glib::wrapper! {
    /// The dialog that shows that an error happened.
    pub struct ErrorDialog(ObjectSubclass<imp::ErrorDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
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
            .property("window", parent)
            .build();
        s.set_response_enabled(REPORT, error.should_report());
        s
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::{CompositeTemplate, UriLauncher};

    use crate::gui::Window;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::ErrorDialog)]
    #[template(resource = "/ui/error_dialog.ui")]
    pub struct ErrorDialog {
        #[property(get, construct_only, set)]
        secondary_error: RefCell<Option<String>>,
        #[property(get, construct_only, set)]
        should_report: Cell<bool>,
        #[property(get, set, construct_only, type = Window)]
        window: RefCell<Option<Window>>,
    }

    #[gtk::template_callbacks]
    impl ErrorDialog {
        #[template_callback]
        fn handle_response(&self, response: &str) {
            log::info!("Response: {}", response);
            if response == super::REPORT {
                let launcher =
                    UriLauncher::new("https://gitlab.com/schmiddi-on-mobile/flare/-/issues");
                launcher.launch(
                    Some(&self.obj().window()),
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
        type ParentType = adw::AlertDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ErrorDialog {}

    impl AdwAlertDialogImpl for ErrorDialog {}
    impl WidgetImpl for ErrorDialog {}
    impl AdwDialogImpl for ErrorDialog {}
}
