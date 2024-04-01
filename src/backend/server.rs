use crate::prelude::*;

use libsignal_service::configuration::SignalServers;

gtk::glib::wrapper! {
    /// Which server to use when linking.
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
        *self.imp().server.borrow()
    }
}

mod imp {
    use crate::prelude::*;

    use libsignal_service::configuration::SignalServers;

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
        const NAME: &'static str = "FlServer";
        type Type = super::Server;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Server {}
}
