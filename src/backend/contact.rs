use std::cell::RefCell;

use gdk::glib::clone;
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{Object, ObjectExt};
use gtk::{gio, glib};
use libsignal_service::prelude::Uuid;
use presage::prelude::ServiceAddress;

use crate::gspawn;

use super::{Channel, Manager};

gtk::glib::wrapper! {
    pub struct Contact(ObjectSubclass<imp::Contact>);
}

impl Contact {
    pub(super) fn from_service_address(address: &ServiceAddress, manager: &Manager) -> Self {
        log::trace!("Building a `Contact` from a `ServiceAddress`");
        if let Ok(Some(contact)) = manager.get_contact_by_id(address.uuid) {
            return Self::from_contact(contact, manager);
        }
        log::trace!("Not in the contact list");
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .build();
        s.imp().uuid.swap(&RefCell::new(Some(address.uuid)));
        s
    }

    pub(super) fn from_contact(contact: presage::prelude::Contact, manager: &Manager) -> Self {
        log::trace!("Building a `Contact` from a `presage::prelude::Contact`");
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .build();
        s.imp()
            .phonenumber
            .swap(&RefCell::new(contact.phone_number.clone()));
        s.imp().uuid.swap(&RefCell::new(Some(contact.uuid)));
        s.imp().contact.swap(&RefCell::new(Some(contact)));
        s
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn is_self(&self) -> bool {
        self.property("is-self")
    }

    pub fn is_blocked(&self) -> bool {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .map(|c| c.blocked)
            .unwrap_or_default()
    }

    pub fn title(&self) -> String {
        self.property("title")
    }

    pub fn channel(&self) -> Option<Channel> {
        self.property("channel")
    }

    pub fn set_channel(&self, channel: Option<&Channel>) {
        self.set_property("channel", channel);
        gspawn!(clone!(@weak self as s => async move {
            s.update_profile_name().await
        }));
    }

    async fn update_profile_name(&self) {
        let obj = self.imp();
        let manager = self.manager();
        let uuid = self.uuid();

        let profile_key = {
            let contact = obj.contact.borrow();
            let channel = self.channel();

            // Don't do anything if contact is self of contact has well-defined name
            if self.is_self()
                || contact
                    .as_ref()
                    .map(|c| !c.name.is_empty())
                    .unwrap_or_default()
            {
                return;
            }

            if let Some(group) = channel.and_then(|c| c.group()) {
                group
                    .members
                    .iter()
                    .filter(|m| m.uuid == uuid)
                    .map(|c| c.profile_key)
                    .next()
            } else {
                contact.as_ref().and_then(|c| c.profile_key().ok())
            }
        };

        if let Some(key) = profile_key {
            // TODO: Error handling?
            let profile = manager.retrieve_profile_by_uuid(uuid, key).await.ok();
            obj.profile.replace(profile);
            self.notify("title");
        }
    }

    pub fn uuid(&self) -> Uuid {
        if let Some(a) = self.address() {
            a.uuid
        } else {
            (*self.imp().uuid.borrow()).unwrap_or_default()
        }
    }

    pub(super) fn address(&self) -> Option<ServiceAddress> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .map(|c| ServiceAddress { uuid: c.uuid })
    }
}

mod imp {
    use std::cell::RefCell;

    use gdk::{prelude::*, subclass::prelude::*};
    use glib::{
        once_cell::sync::Lazy, ParamSpec, ParamSpecBoolean, ParamSpecObject, ParamSpecString, Value,
    };
    use gtk::{gdk, glib};
    use libsignal_service::prelude::Uuid;
    use libsignal_service::Profile;
    use presage::prelude::phonenumber::Mode;

    use crate::backend::{Channel, Manager};

    #[derive(Default)]
    pub struct Contact {
        pub(super) contact: RefCell<Option<presage::prelude::Contact>>,
        pub(super) phonenumber: RefCell<Option<presage::prelude::PhoneNumber>>,
        pub(super) uuid: RefCell<Option<Uuid>>,
        pub(super) profile: RefCell<Option<Profile>>,

        manager: RefCell<Option<Manager>>,
        channel: RefCell<Option<Channel>>,
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
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Channel>("channel").build(),
                    ParamSpecBoolean::builder("is-self").read_only().build(),
                    ParamSpecString::builder("title").read_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "channel" => self.channel.borrow().as_ref().to_value(),
                "is-self" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        (contact.uuid == self.manager.borrow().as_ref().unwrap().uuid()).to_value()
                    } else {
                        false.to_value()
                    }
                }
                "title" => {
                    let contact = self.contact.borrow();
                    let profile = self.profile.borrow();
                    let phonenumber = self.phonenumber.borrow();
                    let uuid = self.uuid.borrow();
                    if self.obj().is_self() {
                        return self
                            .manager
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .profile_name()
                            .to_value();
                    }

                    let contact_title = contact.as_ref().and_then(|c| {
                        if c.name.is_empty() {
                            None
                        } else {
                            Some(c.name.clone())
                        }
                    });
                    let profile_title = profile.as_ref().and_then(|p| p.name.as_ref()).map(|p| {
                        if let Some(family_name) = &p.family_name {
                            format!("{} {}", p.given_name, family_name)
                        } else {
                            p.given_name.clone()
                        }
                    });
                    let phonenumber_title = phonenumber
                        .as_ref()
                        .map(|p| p.format().mode(Mode::National).to_string());
                    let uuid_title = uuid.map(|u| u.to_string());

                    contact_title
                        .or(profile_title)
                        .or(phonenumber_title)
                        .or(uuid_title)
                        // For some reason, Signal includes some special "isolate" control
                        // characters around names with special symbols.
                        .map(|mut s| {
                            s.retain(|c| c != '\u{2068}' && c != '\u{2069}');
                            s
                        })
                        .to_value()
                }
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Contact` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                "channel" => {
                    let obj = value
                        .get::<Option<Channel>>()
                        .expect("Property `channel` of `Contact` has to be of type `Channel`");

                    self.channel.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
