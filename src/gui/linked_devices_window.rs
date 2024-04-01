use crate::prelude::*;

use super::Window;

glib::wrapper! {
    /// Window displaying linked devices (if the device is in primary mode).
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
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    use url::Url;

    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::Window;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::LinkedDevicesWindow)]
    #[template(resource = "/ui/linked_devices_window.ui")]
    pub struct LinkedDevicesWindow {
        #[template_child]
        add_device_dialog: TemplateChild<adw::AlertDialog>,
        #[template_child]
        entry_device_url: TemplateChild<adw::EntryRow>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
        #[property(get, set, construct_only, type = Window)]
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

    #[glib::derived_properties]
    impl ObjectImpl for LinkedDevicesWindow {}

    impl WidgetImpl for LinkedDevicesWindow {}
    impl AdwDialogImpl for LinkedDevicesWindow {}
}
