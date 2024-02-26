use crate::backend::Manager;
use glib::{prelude::ObjectExt, Object};
use gtk::glib;

use super::Window;

glib::wrapper! {
    pub struct LinkedDevicesWindow(ObjectSubclass<imp::LinkedDevicesWindow>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LinkedDevicesWindow {
    pub fn new(manager: Manager, parent: &Window) -> Self {
        log::trace!("Initializing link window");
        Object::builder::<Self>()
            .property("manager", &manager)
            .property("window", parent)
            .build()
    }

    fn manager(&self) -> Manager {
        self.property("manager")
    }

    fn window(&self) -> Window {
        self.property("window")
    }
}

pub mod imp {
    use adw::prelude::WidgetExt;
    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gdk::glib::clone;
    use gdk::prelude::ParamSpecBuilderExt;
    use gdk::prelude::ToValue;
    use glib::{subclass::InitializingObject, ParamSpec, ParamSpecObject, Value};
    use gtk::glib;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;
    use url::Url;

    use crate::backend::Manager;
    use crate::gspawn;
    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::utility::Utility;
    use crate::gui::Window;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/linked_devices_window.ui")]
    pub struct LinkedDevicesWindow {
        // TODO: Port to AlertDialog
        #[template_child]
        add_device_dialog: TemplateChild<adw::AlertDialog>,
        #[template_child]
        entry_device_url: TemplateChild<adw::EntryRow>,

        manager: RefCell<Option<Manager>>,
        window: RefCell<Option<Window>>,
    }

    #[gtk::template_callbacks]
    impl LinkedDevicesWindow {
        #[template_callback]
        fn handle_add_linked_device(&self) {
            log::trace!("User asked to add device link. Presenting dialog.");
            self.entry_device_url.set_text("");
            self.add_device_dialog.present(&self.obj().window());
        }

        #[template_callback]
        fn handle_add_linked_device_response(&self, response: &str) {
            let obj = self.obj();
            self.add_device_dialog.set_visible(false);

            if response == "add-device" {
                let url = self.entry_device_url.text();
                // Should never fail as response is only enabled on success.
                if let Ok(url) = Url::parse(&url) {
                    crate::trace!("User asked to add device link with URL {}.", url);
                    let manager = self.obj().manager();
                    gspawn!(clone!(@weak obj => async move {
                        if let Err(e) = manager.link_secondary(url).await {
                            let root = obj.window();
                            let dialog = ErrorDialog::new(e.into(), &root);
                            dialog.present(&root);
                        }
                    }));
                }
            }
        }

        #[template_callback]
        fn handle_url_entry_changed(&self) {
            self.add_device_dialog.set_response_enabled(
                "add-device",
                Url::parse(&self.entry_device_url.text()).is_ok(),
            );
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LinkedDevicesWindow {
        const NAME: &'static str = "FlLinkedDevicesWindow";
        type Type = super::LinkedDevicesWindow;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LinkedDevicesWindow {
        fn constructed(&self) {
            log::trace!("Constructed LinkedDevicesWindow");
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Window>("window")
                        .construct_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "window" => self.window.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value.get::<Option<Manager>>().expect(
                        "Property `manager` of `LinkedDevicesWindow` has to be of type `Manager`",
                    );

                    self.manager.replace(man);
                }
                "window" => {
                    let win = value.get::<Option<Window>>().expect(
                        "Property `window` of `LinkedDevicesWindow` has to be of type `Window`",
                    );

                    self.window.replace(win);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for LinkedDevicesWindow {}
    impl AdwDialogImpl for LinkedDevicesWindow {}
}
