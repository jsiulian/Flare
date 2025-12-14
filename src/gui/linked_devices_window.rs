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
        let s = Object::builder::<Self>()
            .property("manager", &manager)
            .property("window", parent)
            .build();
        gspawn!(clone!(
            #[weak]
            s,
            async move { s.imp().setup().await }
        ));
        s
    }
}

pub mod imp {
    use crate::backend::DeviceInfo;
    use crate::gui::device_info_item::DeviceInfoItem;
    use crate::prelude::*;

    use adw::{AlertDialog, ResponseAppearance};
    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;
    use gtk::gio::ListStore;

    use url::Url;

    use crate::gui::Window;
    use crate::gui::error_dialog::ErrorDialog;

    #[derive(CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LinkedDevicesWindow)]
    #[template(resource = "/ui/linked_devices_window.ui")]
    pub struct LinkedDevicesWindow {
        #[template_child]
        add_device_dialog: TemplateChild<adw::AlertDialog>,
        #[template_child]
        entry_device_url: TemplateChild<adw::EntryRow>,

        #[template_child]
        list_devices: TemplateChild<gtk::ListBox>,
        model_devices: RefCell<ListStore>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
        #[property(get, set, construct_only, type = Window)]
        window: RefCell<Option<Window>>,
    }

    impl Default for LinkedDevicesWindow {
        fn default() -> Self {
            Self {
                add_device_dialog: Default::default(),
                entry_device_url: Default::default(),
                list_devices: Default::default(),
                model_devices: RefCell::new(ListStore::new::<DeviceInfo>()),
                manager: Default::default(),
                window: Default::default(),
            }
        }
    }

    #[gtk::template_callbacks]
    impl LinkedDevicesWindow {
        pub(super) async fn setup(&self) {
            let obj = self.obj();

            self.list_devices.bind_model(
                Some(&*self.model_devices.borrow()),
                clone!(
                    #[weak]
                    obj,
                    #[upgrade_or_panic]
                    move |item| {
                        let device_item = DeviceInfoItem::new();
                        device_item.set_device_info(
                            item.dynamic_cast_ref::<DeviceInfo>()
                                .expect("Device list to contain DeviceInfo items"),
                        );

                        device_item.connect_local(
                            "unlink",
                            false,
                            clone!(
                                #[weak]
                                obj,
                                #[weak]
                                device_item,
                                #[upgrade_or_default]
                                move |_| {
                                    if let Some(device) = device_item.device_info() {
                                        obj.imp().unlink(&device);
                                    }
                                    None
                                }
                            ),
                        );

                        device_item.into()
                    }
                ),
            );

            self.reload().await;
        }

        fn unlink(&self, device: &DeviceInfo) {
            let manager = self.obj().manager();
            let obj = self.obj();
            gspawn!(clone!(
                #[weak]
                obj,
                #[weak]
                manager,
                #[weak]
                device,
                async move {
                    let dialog = AlertDialog::new(
                        Some(&gettextrs::gettext("Unlink Device?")),
                        Some(
                            &gettextrs::gettext("Are you sure you want to unlink {}?")
                                .replace("{}", &device.name()),
                        ),
                    );
                    dialog.add_responses(&[
                        ("unlink", &gettextrs::gettext("Unlink")),
                        ("cancel", &gettextrs::gettext("Cancel")),
                    ]);
                    dialog.set_response_appearance("unlink", ResponseAppearance::Destructive);
                    dialog.set_default_response(Some("cancel"));
                    let response = dialog.choose_future(Some(&obj)).await;
                    if response == "unlink" {
                        log::debug!("Unlinking device: {} {}", device.id(), device.name());
                        if let Err(e) = manager.unlink_secondary(device.id()).await {
                            log::error!("Failed to unlink device: {}", e);
                        }
                        println!("Unlink {}", device.name());
                        obj.imp().reload().await;
                    }
                }
            ));
        }

        async fn reload(&self) {
            let manager = self.obj().manager();
            let linked_devices = manager
                .devices()
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|d| d.name.is_some())
                .map(DeviceInfo::new)
                .collect::<Vec<_>>();

            let model = self.model_devices.borrow();
            model.remove_all();
            model.extend_from_slice(&linked_devices);
        }

        #[template_callback]
        fn handle_add_linked_device(&self) {
            log::trace!("User asked to add device link. Presenting dialog.");
            self.entry_device_url.set_text("");
            self.add_device_dialog.present(Some(&self.obj().window()));
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
                    gspawn!(clone!(
                        #[weak]
                        obj,
                        async move {
                            if let Err(e) = manager.link_secondary(url).await {
                                let root = obj.window();
                                let dialog = ErrorDialog::new(&e.into(), &root);
                                dialog.present(Some(&root));
                            }
                        }
                    ));
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
