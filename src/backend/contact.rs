use std::cell::RefCell;

use gdk_pixbuf::glib::Object;
use gio::subclass::prelude::ObjectSubclassIsExt;
use presage::prelude::ServiceAddress;

use super::Manager;

gtk::glib::wrapper! {
    pub struct Contact(ObjectSubclass<imp::Contact>);
}

impl Contact {
    pub(super) fn from_service_address(address: &ServiceAddress, manager: &Manager) -> Self {
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Contact`");
        s.imp()
            .phonenumber
            .swap(&RefCell::new(address.phonenumber.clone()));
        if let Some(uuid) = address.uuid {
            let contact = manager.internal().get_contact_by_id(uuid);
            s.imp().contact.swap(&RefCell::new(contact.ok().flatten()));
        }
        return s;
    }

    pub(super) fn from_contact(contact: presage::prelude::Contact, manager: &Manager) -> Self {
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Contact`");
        s.imp()
            .phonenumber
            .swap(&RefCell::new(contact.address.phonenumber.clone()));
        s.imp().contact.swap(&RefCell::new(Some(contact)));
        return s;
    }

    pub(super) fn address(&self) -> Option<ServiceAddress> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .map(|c| c.address.clone())
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk_pixbuf::{
        glib::{
            once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject,
            ParamSpecString, Value,
        },
        prelude::{StaticType, ToValue},
    };
    use gtk::glib;
    use presage::prelude::phonenumber::Mode;
    use std::cell::RefCell;

    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Contact {
        pub(super) contact: RefCell<Option<presage::prelude::Contact>>,
        pub(super) phonenumber: RefCell<Option<presage::prelude::PhoneNumber>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Contact {
        const NAME: &'static str = "FlContact";
        type Type = super::Contact;
    }

    impl ObjectImpl for Contact {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecBoolean::new(
                        "is-self",
                        "is-self",
                        "is-self",
                        false,
                        ParamFlags::READABLE,
                    ),
                    ParamSpecString::new("title", "title", "title", None, ParamFlags::READABLE),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "is-self" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        let name = &contact.name;
                        if name.is_empty() {
                            true.to_value()
                        } else {
                            false.to_value()
                        }
                    } else {
                        false.to_value()
                    }
                }
                "title" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        let name = &contact.name;
                        if name.is_empty() {
                            self.manager
                                .borrow()
                                .as_ref()
                                .expect("`Manager` of `Contact` to be set up")
                                .profile_name()
                                .to_value()
                        } else {
                            name.to_value()
                        }
                    } else if let Some(phone) = self.phonenumber.borrow().as_ref() {
                        phone.format().mode(Mode::National).to_string().to_value()
                    } else {
                        "".to_value()
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Contact` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
