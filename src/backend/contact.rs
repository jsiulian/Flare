use std::cell::RefCell;

use gdk::{
    glib::{clone, Bytes},
    Paintable, Texture,
};
use gio::subclass::prelude::ObjectSubclassIsExt;
use glib::{prelude::ObjectExt, Object};
use gtk::{gio, glib};
use libsignal_service::{
    prelude::{phonenumber::Mode, Uuid},
    ServiceAddress,
};

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

    pub(super) fn from_contact(
        contact: libsignal_service::models::Contact,
        manager: &Manager,
    ) -> Self {
        log::trace!("Building a `Contact` from a `libsignal_service::models::Contact`");
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
            let Some(avatar) = manager
                .retrieve_profile_avatar_by_uuid(uuid, key)
                .await
                .ok()
                .flatten()
                .and_then(|b| Texture::from_bytes(&Bytes::from_owned(b)).ok())
            else {
                log::debug!(
                    "Failed to fetch avatar for {}; they may not have a profile picture set",
                    self.title()
                );
                return;
            };

            self.set_avatar(avatar);
        }
    }

    pub fn avatar(&self) -> Option<Paintable> {
        self.property("avatar")
    }

    fn set_avatar(&self, image: Texture) {
        self.set_property("avatar", image);
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

    pub fn expire_timer(&self) -> u32 {
        // Unwrap should never happen.
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .map(|c| c.expire_timer)
            .unwrap_or_default()
    }

    pub fn phone_number(&self) -> Option<String> {
        self.imp()
            .contact
            .borrow()
            .as_ref()
            .and_then(|c| c.phone_number.as_ref().map(|p| p.to_string()))
    }

    pub fn description(&self) -> Option<String> {
        self.imp().profile.borrow().as_ref().and_then(|c| {
            // Should be fixed upstream
            let emoji = c.about_emoji.clone().unwrap_or_default();
            let about = c.about.clone().unwrap_or_default();
            match (emoji.as_str(), about.as_str()) {
                ("", "") => None,
                ("", about) => Some(about.to_string()),
                (emoji, "") => Some(emoji.to_string()),
                (emoji, about) => Some(format!("{} {}", emoji, about)),
            }
        })
    }

    pub fn name_parts(&self) -> (Option<String>, String) {
        if self.is_self() {
            return (None, self.manager().profile_name());
        }

        let contact_title = self.imp().contact.borrow().as_ref().and_then(|c| {
            if c.name.is_empty() {
                None
            } else if let Some((p1, p2)) = c.name.rsplit_once(' ') {
                Some((Some(p1.to_owned()), p2.to_owned()))
            } else {
                Some((None, c.name.clone()))
            }
        });
        let profile_title = || {
            self.imp()
                .profile
                .borrow()
                .as_ref()
                .and_then(|p| p.name.as_ref())
                .map(|n| {
                    if let Some(family) = &n.family_name {
                        (Some(n.given_name.clone()), family.clone())
                    } else {
                        (None, n.given_name.clone())
                    }
                })
        };
        let phonenumber_title = || {
            self.imp()
                .phonenumber
                .borrow()
                .as_ref()
                .map(|p| (None, p.format().mode(Mode::National).to_string()))
        };

        let (mut s1, mut s2) = contact_title
            .or_else(profile_title)
            .or_else(phonenumber_title)
            .unwrap_or_else(|| (None, gettextrs::gettext("Unknown contact")));
        // For some reason, Signal includes some special "isolate" control
        // characters around names with special symbols.
        if let Some(s1) = &mut s1 {
            s1.retain(|c| c != '\u{2068}' && c != '\u{2069}')
        }
        s2.retain(|c| c != '\u{2068}' && c != '\u{2069}');
        (s1, s2)
    }
}

mod imp {
    use std::cell::RefCell;

    use gdk::Paintable;
    use gdk::{prelude::*, subclass::prelude::*};
    use glib::{ParamSpec, ParamSpecBoolean, ParamSpecObject, ParamSpecString, Value};
    use gtk::{gdk, glib};
    use libsignal_service::prelude::phonenumber::Mode;
    use libsignal_service::prelude::{phonenumber::PhoneNumber, Uuid};
    use libsignal_service::Profile;
    use once_cell::sync::Lazy;

    use crate::backend::{Channel, Manager};

    #[derive(Default)]
    pub struct Contact {
        pub(super) contact: RefCell<Option<libsignal_service::models::Contact>>,
        pub(super) phonenumber: RefCell<Option<PhoneNumber>>,
        pub(super) uuid: RefCell<Option<Uuid>>,
        pub(super) profile: RefCell<Option<Profile>>,

        pub(crate) avatar: RefCell<Option<Paintable>>,

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
                    ParamSpecObject::builder::<Paintable>("avatar").build(),
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
                "avatar" => self.avatar.borrow().as_ref().to_value(),
                "is-self" => {
                    if let Some(contact) = self.contact.borrow().as_ref() {
                        (contact.uuid == self.manager.borrow().as_ref().unwrap().uuid()).to_value()
                    } else {
                        false.to_value()
                    }
                }
                "title" => {
                    if self.obj().is_self() {
                        return self
                            .manager
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .profile_name()
                            .to_value();
                    }

                    let contact_title = self.contact.borrow().as_ref().and_then(|c| {
                        if c.name.is_empty() {
                            None
                        } else {
                            Some(c.name.clone())
                        }
                    });
                    let profile_title = || {
                        self.profile
                            .borrow()
                            .as_ref()
                            .and_then(|p| p.name.as_ref())
                            .map(crate::utils::format_profile_name)
                    };
                    let phonenumber_title = || {
                        self.phonenumber
                            .borrow()
                            .as_ref()
                            .map(|p| p.format().mode(Mode::National).to_string())
                    };

                    contact_title
                        .or_else(profile_title)
                        .or_else(phonenumber_title)
                        .or_else(|| Some(gettextrs::gettext("Unknown contact")))
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
                "avatar" => {
                    let obj = value
                        .get::<Option<Paintable>>()
                        .expect("Property `avatar` of `Contact` has to be of type `Paintable`");

                    self.avatar.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
