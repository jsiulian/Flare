use std::cell::RefCell;

use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{Object, ObjectExt};
use presage::prelude::ServiceAddress;

use super::Manager;

gtk::glib::wrapper! {
    pub struct Contact(ObjectSubclass<imp::Contact>);
}

impl Contact {
    pub(super) fn from_service_address(address: &ServiceAddress, manager: &Manager) -> Self {
        log::trace!("Building a `Contact` from a `ServiceAddress`");
        if let Some(uuid) = address.uuid {
            if let Ok(Some(contact)) = manager.get_contact_by_id(uuid) {
                return Self::from_contact(contact, manager);
            }
        }
        log::trace!("Not in the contact list");
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Contact`");
        s.imp()
            .phonenumber
            .swap(&RefCell::new(address.phonenumber.clone()));
        s.imp().address.swap(&RefCell::new(Some(address.clone())));
        s
    }

    pub(super) fn from_contact(contact: presage::prelude::Contact, manager: &Manager) -> Self {
        log::trace!("Building a `Contact` from a `presage::prelude::Contact`");
        let s: Self = Object::new(&[("manager", manager)]).expect("Failed to create `Contact`");
        s.imp()
            .phonenumber
            .swap(&RefCell::new(contact.address.phonenumber.clone()));
        s.imp()
            .address
            .swap(&RefCell::new(Some(contact.address.clone())));
        s.imp().contact.swap(&RefCell::new(Some(contact)));
        s
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn is_self(&self) -> bool {
        self.property("is-self")
    }

    pub fn title(&self) -> String {
        self.property("title")
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
    use std::cell::RefCell;

    use gdk::{prelude::*, subclass::prelude::*};
    use glib::{
        once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject,
        ParamSpecString, Value,
    };
    use presage::prelude::phonenumber::Mode;

    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Contact {
        pub(super) contact: RefCell<Option<presage::prelude::Contact>>,
        pub(super) phonenumber: RefCell<Option<presage::prelude::PhoneNumber>>,
        pub(super) address: RefCell<Option<presage::prelude::ServiceAddress>>,

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

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "is-self" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        (contact.address.uuid == Some(obj.manager().uuid())).to_value()
                    } else {
                        false.to_value()
                    }
                }
                "title" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        let name = &contact.name;
                        if obj.is_self() {
                            obj.manager().profile_name().to_value()
                        } else if name.is_empty() {
                            if let Some(phone) = self.phonenumber.borrow().as_ref() {
                                phone.format().mode(Mode::National).to_string().to_value()
                            } else {
                                contact.address.uuid.map(|u| u.to_string()).to_value()
                            }
                        } else {
                            name.to_value()
                        }
                    } else if let Some(phone) = self.phonenumber.borrow().as_ref() {
                        phone.format().mode(Mode::National).to_string().to_value()
                    } else if let Some(address) = self.address.borrow().as_ref() {
                        address
                            .phonenumber
                            .as_ref()
                            .map(|p| p.format().mode(Mode::National).to_string())
                            .or_else(|| address.uuid.map(|u| u.to_string()))
                            .to_value()
                    } else {
                        None::<String>.to_value()
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
