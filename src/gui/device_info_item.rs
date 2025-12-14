use crate::prelude::*;

glib::wrapper! {
    /// An item modelling a device in the [crate::gui::LinkedDevicesWindow].
    pub struct DeviceInfoItem(ObjectSubclass<imp::DeviceInfoItem>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Actionable;
}

impl DeviceInfoItem {
    pub fn new() -> Self {
        log::trace!("Initializing `DeviceInfoItem`");
        Object::builder::<Self>().build()
    }
}

impl Default for DeviceInfoItem {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use crate::{backend::DeviceInfo, prelude::*};

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::DeviceInfoItem)]
    #[template(resource = "/ui/device_info_item.ui")]
    pub struct DeviceInfoItem {
        #[property(get, set)]
        device_info: RefCell<Option<DeviceInfo>>,
    }

    #[gtk::template_callbacks]
    impl DeviceInfoItem {
        #[template_callback]
        fn unlink(&self) {
            self.obj().emit_by_name::<()>("unlink", &[]);
        }

        #[template_callback(function)]
        fn format_subtitle(created: String, last_seen: String) -> String {
            let created_format = gettextrs::gettext("Linked: {}").replace("{}", &created);
            let last_seen_format = gettextrs::gettext("Last Seen: {}").replace("{}", &last_seen);
            format!("{created_format}\n{last_seen_format}")
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceInfoItem {
        const NAME: &'static str = "FlDeviceInfoItem";
        type Type = super::DeviceInfoItem;
        type ParentType = adw::ActionRow;

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
    impl ObjectImpl for DeviceInfoItem {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| -> Vec<Signal> { vec![Signal::builder("unlink").build()] });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for DeviceInfoItem {}
    impl ListBoxRowImpl for DeviceInfoItem {}
    impl PreferencesRowImpl for DeviceInfoItem {}
    impl ActionRowImpl for DeviceInfoItem {}
}
