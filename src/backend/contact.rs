use crate::prelude::*;

use gdk::Texture;
use glib::Bytes;
use libsignal_service::{
    prelude::{phonenumber::Mode, Uuid},
    ServiceAddress,
};

use super::Manager;

gtk::glib::wrapper! {
    /// A contact represents a user of Signal.
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

    pub fn is_blocked(&self) -> bool {
        // TODO: The blocked field got removed in https://github.com/whisperfish/libsignal-service-rs/pull/303
        false
    }

    /// Query the profile name and avatar of the contact.
    pub async fn update_profile_name_and_avatar(&self) {
        let obj = self.imp();
        let manager = self.manager();
        let uuid = self.uuid();

        let profile_key = {
            let contact = obj.contact.borrow();
            let channel = self.channel();

            // Note: The profile key may be different if the contact is in a group or a 1-to-1 message.
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

            // Avatar
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
            .map(|c| ServiceAddress::new_aci(c.uuid))
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

    /// A heuristic how to split the name.
    ///
    /// Note that this may not be the best solution, e.g. for persons with multi-part surname.
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
    use crate::prelude::*;

    use std::marker::PhantomData;

    use gdk::Paintable;
    use libsignal_service::prelude::phonenumber::{Mode, PhoneNumber};
    use libsignal_service::Profile;

    use crate::backend::{Channel, Manager};

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Contact)]
    pub struct Contact {
        pub(super) contact: RefCell<Option<libsignal_service::models::Contact>>,
        pub(super) phonenumber: RefCell<Option<PhoneNumber>>,
        pub(super) uuid: RefCell<Option<Uuid>>,
        pub(super) profile: RefCell<Option<Profile>>,

        #[property(get, set)]
        pub(crate) avatar: RefCell<Option<Paintable>>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
        #[property(get, set, nullable)]
        channel: RefCell<Option<Channel>>,

        #[property(get = Self::is_self)]
        is_self: PhantomData<bool>,
        #[property(get = Self::title)]
        title: PhantomData<String>,
    }

    impl Contact {
        fn is_self(&self) -> bool {
            if let Some(contact) = self.contact.borrow().as_ref() {
                contact.uuid == self.manager.borrow().as_ref().unwrap().uuid()
            } else {
                false
            }
        }

        fn title(&self) -> String {
            if self.obj().is_self() {
                return self.manager.borrow().as_ref().unwrap().profile_name();
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
                .unwrap_or_default()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Contact {
        const NAME: &'static str = "FlContact";
        type Type = super::Contact;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Contact {}
}
