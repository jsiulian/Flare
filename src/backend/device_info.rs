use crate::prelude::*;

gtk::glib::wrapper! {
    /// Info about a linked device.
    pub struct DeviceInfo(ObjectSubclass<imp::DeviceInfo>);
}

impl DeviceInfo {
    pub fn new(info: libsignal_service::push_service::DeviceInfo) -> DeviceInfo {
        Object::builder::<Self>()
            .property("id", u8::from(info.id))
            .property("name", info.name)
            .property(
                "created",
                crate::utils::chrono_to_glib_datetime(info.created_at)
                    .and_then(|d| Utility::format_date_human(&d))
                    .unwrap_or_default(),
            )
            .property(
                "last-seen",
                crate::utils::chrono_to_glib_datetime(info.last_seen)
                    .and_then(|d| Utility::format_date_human(&d))
                    .unwrap_or_default(),
            )
            .build()
    }
}

mod imp {
    use crate::prelude::*;

    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type=super::DeviceInfo)]
    pub struct DeviceInfo {
        #[property(get, set)]
        id: RefCell<u8>,
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set)]
        created: RefCell<String>,
        #[property(get, set)]
        last_seen: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceInfo {
        const NAME: &'static str = "FlDeviceInfo";
        type Type = super::DeviceInfo;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceInfo {}
}
