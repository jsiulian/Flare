use gdk::glib::Object;
use gdk::subclass::prelude::ObjectSubclassIsExt;
use libsignal_service::configuration::SignalServers;

gtk::glib::wrapper! {
    pub struct Server(ObjectSubclass<imp::Server>);
}

impl Server {
    pub fn new(display: String, value: SignalServers) -> Server {
        let o = Object::builder::<Self>()
            .property("display", display)
            .build();
        o.imp().server.replace(value);
        o
    }

    pub fn server(&self) -> SignalServers {
        self.imp().server.borrow().clone()
    }
}

mod imp {
    use gtk::glib;
    use libsignal_service::configuration::SignalServers;
    use std::cell::RefCell;

    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};

    use gtk::prelude::ObjectExt;
    use gtk::subclass::prelude::DerivedObjectProperties;

    #[derive(glib::Properties)]
    #[properties(wrapper_type=super::Server)]
    pub struct Server {
        #[property(get, set)]
        display: RefCell<String>,

        pub(super) server: RefCell<SignalServers>,
    }

    impl Default for Server {
        fn default() -> Self {
            Self {
                display: Default::default(),
                server: RefCell::new(SignalServers::Production),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Server {
        const NAME: &'static str = "DBServer";
        type Type = super::Server;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Server {}
}
