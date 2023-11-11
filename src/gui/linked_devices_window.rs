use crate::backend::Manager;
use adw::prelude::GtkWindowExt;
use gdk::prelude::CastNone;
use glib::{Object, ObjectExt};
use gtk::glib;

use super::Window;

glib::wrapper! {
    pub struct LinkedDevicesWindow(ObjectSubclass<imp::LinkedDevicesWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl LinkedDevicesWindow {
    pub fn new(manager: Manager, parent: &Window) -> Self {
        log::trace!("Initializing link window");
        Object::builder::<Self>()
            .property("manager", &manager)
            .property("transient-for", parent)
            .build()
    }

    fn manager(&self) -> Manager {
        self.property("manager")
    }

    fn window(&self) -> Window {
        self.transient_for()
            .and_dynamic_cast()
            .expect("LinkedDevicesWindow to have a Window parent")
    }
}

pub mod imp {
    use adw::prelude::EditableExt;
    use adw::prelude::GtkWindowExt;
    use adw::prelude::{MessageDialogExt, WidgetExt};
    use adw::subclass::prelude::*;
    use gdk::glib::clone;
    use gdk::prelude::ParamSpecBuilderExt;
    use gdk::prelude::ToValue;
    use glib::{
        once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecObject, Value,
    };
    use gtk::glib;
    use gtk::CompositeTemplate;
    use std::cell::RefCell;
    use url::Url;

    use crate::backend::Manager;
    use crate::gspawn;
    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::utility::Utility;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/linked_devices_window.ui")]
    pub struct LinkedDevicesWindow {
        #[template_child]
        add_device_dialog: TemplateChild<adw::MessageDialog>,
        #[template_child]
        entry_device_url: TemplateChild<adw::EntryRow>,

        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl LinkedDevicesWindow {
        #[template_callback]
        fn handle_add_linked_device(&self) {
            log::trace!("User asked to add device link. Presenting dialog.");
            self.entry_device_url.set_text("");
            self.add_device_dialog
                .set_transient_for(Some(&self.obj().window()));
            self.add_device_dialog.present();
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
                            let dialog = ErrorDialog::new(e.into(), &obj.window());
                            dialog.present();
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
        type ParentType = adw::Window;

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
                vec![ParamSpecObject::builder::<Manager>("manager")
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
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
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for LinkedDevicesWindow {}
    impl WindowImpl for LinkedDevicesWindow {}
    impl AdwWindowImpl for LinkedDevicesWindow {}
}
