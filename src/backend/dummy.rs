use std::cell::OnceCell;
use std::path::Path;

use gtk::glib::{BoxedAnyObject, DateTime};
use gtk::prelude::{Cast, FileExt};
use gtk::{prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};
use libsignal_service::Profile;
use libsignal_service::content::CallMessage as PreCallMessage;
use libsignal_service::prelude::AttachmentPointer;
use libsignal_service::prelude::Content;
use libsignal_service::prelude::ProfileKey;
use libsignal_service::prelude::Uuid;
use libsignal_service::proto::GroupContextV2;
use libsignal_service::proto::call_message::Hangup;
use libsignal_service::proto::call_message::Offer;
use libsignal_service::proto::data_message::Reaction;
use libsignal_service::protocol::DeviceId;
use libsignal_service::sender::AttachmentSpec;
use libsignal_service::sender::AttachmentUploadError;
use libsignal_service::websocket::account::DeviceInfo;
use presage::model::contacts::Contact as LContact;
use presage::model::groups::Group;
use presage::store::Thread;

use super::{
    Channel, Contact,
    message::{CallMessage, Message, MessageExt, ReactionMessage, TextMessage},
};
use crate::backend::timeline::timeline_item::TimelineItemExt;
use crate::{backend::SetupResult, error::ApplicationError};

const GROUP_ID: usize = 5;
type PresageError = presage::Error<presage_store_sqlite::SqliteStoreError>;

macro_rules! msg {
    ($s:expr, $m:expr, $i:expr, $j:expr, $t:expr) => {
        TextMessage::pub_from_text_channel_sender_timestamp(
            $m,
            $s.dummy_channels().await[$j].clone(),
            $s.dummy_contacts().await[$i].clone(),
            $t * 1000 * 60,
            $s,
        )
        .upcast::<Message>()
    };
    ($s:expr, $m:expr, $i:expr, $t:expr) => {
        msg!($s, $m, $i, 1, $t)
    };
}

macro_rules! call_msg {
    ($s:expr, $m:expr, $i:expr, $t:expr) => {{
        let c = $s.dummy_contacts().await[$i].clone();
        CallMessage::from_call(
            &c,
            &Channel::from_contact_or_group(c.clone(), &None, $s).await,
            $t * 1000 * 60,
            $s,
            $m,
        )
        .expect("`CallMessage` to be valid")
        .upcast::<Message>()
    }};
}

pub fn dummy_presage_contacts() -> Vec<LContact> {
    vec![
        LContact {
            uuid: Uuid::from_u128(0),
            phone_number: None,
            name: "".to_string(),
            verified: Default::default(),
            profile_key: vec![0; 32],
            expire_timer: 0,
            inbox_position: 0,
            avatar: None,
            expire_timer_version: 0,
        },
        LContact {
            uuid: Uuid::from_u128(1),
            phone_number: None,
            name: "Postmarket OS Linux Mobile User".to_string(),
            verified: Default::default(),
            profile_key: vec![0; 32],
            expire_timer: 0,
            inbox_position: 0,
            avatar: None,
            expire_timer_version: 0,
        },
        LContact {
            uuid: Uuid::from_u128(2),
            phone_number: Some(
                phonenumber::parse(None, "+123456789")
                    .expect("Developer to have a valid phone number"),
            ),
            name: "Developer".to_string(),
            verified: Default::default(),
            profile_key: vec![0; 32],
            expire_timer: 0,
            inbox_position: 0,
            avatar: None,
            expire_timer_version: 0,
        },
        LContact {
            uuid: Uuid::from_u128(3),
            phone_number: None,
            name: "Admrial Ackbar".to_string(),
            verified: Default::default(),
            profile_key: vec![0; 32],
            expire_timer: 0,
            inbox_position: 0,
            avatar: None,
            expire_timer_version: 0,
        },
        LContact {
            uuid: Uuid::from_u128(4),
            phone_number: None,
            name: "Agent Smith".to_string(),
            verified: Default::default(),
            profile_key: vec![0; 32],
            expire_timer: 0,
            inbox_position: 0,
            avatar: None,
            expire_timer_version: 0,
        },
    ]
}

impl super::Manager {
    #[cfg(feature = "screenshot")]
    pub async fn init<P: AsRef<Path>>(&self, _p: &P) -> Result<(), crate::ApplicationError> {
        log::trace!("Init manager for screenshots");
        self.init_channels().await;
        self.setup_receive_message_loop().await?;

        self.imp().finished_setup.set(true);
        self.notify("finished-setup");

        #[cfg(feature = "screenshot-setup")]
        {
            let (tx_decision, rx_decision) = futures::channel::oneshot::channel();
            let to_send = OnceCell::new();
            let _ = to_send.set(tx_decision);
            self.emit_by_name::<()>(
                "setup-result",
                &[&BoxedAnyObject::new(SetupResult::Pending(to_send))],
            );
            let _ = rx_decision.await;
            self.emit_by_name::<()>(
                "setup-result",
                &[&BoxedAnyObject::new(SetupResult::DisplayLinkQR(
                    "https://mobile.schmidhuberj.de/".try_into().unwrap(),
                ))],
            );
            gtk::glib::timeout_future_seconds(2).await;
            self.emit_by_name::<()>(
                "setup-result",
                &[&BoxedAnyObject::new(SetupResult::Finished)],
            );
        }

        Ok(())
    }

    #[cfg(feature = "screenshot")]
    pub async fn setup_receive_message_loop(&self) -> Result<(), PresageError> {
        log::trace!("Setup receive loop for screenshots");
        let channels = self.imp().channels.borrow();

        for msg in self.dummy_messages().await {
            self.emit_by_name::<()>("message", &[&msg]);
            if let Some(stored_channel) = channels.get(&msg.channel().thread()) {
                log::debug!("Message from a already existing channel");
                let _ = stored_channel.new_message(msg).await;
            }
        }
        Ok(())
    }

    #[cfg(feature = "screenshot")]
    pub fn profile_name(&self) -> String {
        "You".to_string()
    }

    #[cfg(feature = "screenshot")]
    pub async fn message(
        &self,
        thread: &Thread,
        timestamp: u64,
    ) -> Result<Option<Message>, ApplicationError> {
        Ok(self
            .dummy_messages()
            .await
            .into_iter()
            .find(|m| m.timestamp() == timestamp))
    }

    #[cfg(feature = "screenshot")]
    pub async fn messages(
        &self,
        thread: &Thread,
        from: Option<u64>,
    ) -> Result<impl Iterator<Item = Content>, ApplicationError> {
        Ok(std::iter::empty())
    }

    #[cfg(feature = "screenshot")]
    pub(super) fn uuid(&self) -> Uuid {
        Uuid::nil()
    }

    #[cfg(feature = "screenshot")]
    pub(super) async fn retrieve_profile_by_uuid(
        &self,
        _uuid: Uuid,
        _profile_key: ProfileKey,
    ) -> Result<Profile, PresageError> {
        Ok(Profile {
            name: None,
            about: Some(
                "I am the Developer. I thus may write here what I want. (Insert evil laughter)\nLorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet."
                    .to_string(),
            ),
            about_emoji: Some("🤯️".to_string()),
            avatar: None,
            unrestricted_unidentified_access: false,
        })
    }

    #[cfg(feature = "screenshot")]
    pub(super) async fn retrieve_profile_avatar_by_uuid(
        &self,
        uuid: Uuid,
        _profile_key: ProfileKey,
    ) -> Result<Option<Vec<u8>>, PresageError> {
        if uuid == Uuid::from_u128(2) {
            let screenshot_file = gtk::gio::File::for_uri("resource:///icon.svg");
            Ok(Some(
                screenshot_file
                    .load_bytes(None::<gtk::gio::Cancellable>.as_ref())
                    .expect("Failed to load icon bytes")
                    .0
                    .to_vec(),
            ))
        } else {
            Ok(None)
        }
    }

    #[cfg(feature = "screenshot")]
    pub(super) async fn get_profile_key_by_id(
        &self,
        id: libsignal_service::protocol::ServiceId,
    ) -> Result<Option<libsignal_service::prelude::ProfileKey>, ApplicationError> {
        Ok(dummy_presage_contacts()
            .into_iter()
            .find(|c| c.uuid == id.raw_uuid())
            .and_then(|c| c.profile_key.try_into().ok())
            .map(|p| ProfileKey { bytes: p }))
    }

    #[cfg(feature = "screenshot")]
    pub(super) async fn get_contact_by_id(
        &self,
        id: libsignal_service::protocol::ServiceId,
    ) -> Result<Option<presage::model::contacts::Contact>, ApplicationError> {
        Ok(dummy_presage_contacts()
            .into_iter()
            .find(|c| c.uuid == id.raw_uuid()))
    }

    #[cfg(feature = "screenshot")]
    pub(super) async fn retrieve_group_avatar(
        &self,
        context: GroupContextV2,
    ) -> Result<Option<Vec<u8>>, PresageError> {
        Ok(None)
    }

    #[cfg(feature = "screenshot")]
    pub async fn devices(&self) -> Result<Vec<DeviceInfo>, PresageError> {
        use chrono::{TimeDelta, Utc};

        let now = Utc::now();
        let base_time = now
            .with_time(chrono::NaiveTime::from_hms_opt(13, 24, 0).unwrap())
            .unwrap();

        Ok(vec![
            DeviceInfo {
                id: DeviceId::new(1).unwrap(),
                registration_id: 0,
                name: Some("Flare (Desktop)".to_string()),
                created_at: base_time - TimeDelta::days(10),
                last_seen: base_time - TimeDelta::days(1),
            },
            DeviceInfo {
                id: DeviceId::new(2).unwrap(),
                registration_id: 0,
                name: Some("Flare (PinePhone)".to_string()),
                created_at: base_time - TimeDelta::days(7),
                last_seen: base_time,
            },
            DeviceInfo {
                id: DeviceId::new(3).unwrap(),
                registration_id: 0,
                name: Some("Flare (Another)".to_string()),
                created_at: base_time - TimeDelta::days(7),
                last_seen: base_time,
            },
            DeviceInfo {
                id: DeviceId::new(4).unwrap(),
                registration_id: 0,
                name: Some("Flare (Another2)".to_string()),
                created_at: base_time - TimeDelta::days(7),
                last_seen: base_time,
            },
            DeviceInfo {
                id: DeviceId::new(5).unwrap(),
                registration_id: 0,
                name: Some("Flare (Another3)".to_string()),
                created_at: base_time - TimeDelta::days(7),
                last_seen: base_time,
            },
        ])
    }

    #[cfg(feature = "screenshot")]
    pub fn is_primary(&self) -> bool {
        true
    }

    #[cfg(feature = "screenshot")]
    pub async fn upload_attachments(
        &self,
        attachments: Vec<(AttachmentSpec, Vec<u8>)>,
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, PresageError> {
        Ok(vec![Ok(AttachmentPointer::default())])
    }

    #[cfg(feature = "screenshot")]
    async fn dummy_messages(&self) -> Vec<Message> {
        let now = DateTime::now_utc().expect("Now to be expressable as DateTime");
        let base_time = DateTime::from_utc(now.year(), now.month(), now.day_of_month(), 11, 0, 0.0)
            .expect("Base time to be expressable as DateTime");
        let base_minute: u64 = (base_time.to_unix() / 60).try_into().unwrap();

        let msg_replied = msg!(
            self,
            "Flare 0.19.0 was now released. This release will now show a loading screen on startup while it receives messages. Besides that, there have also been many minor UI changes, like a reduced spacing between reactions and their corresponding mesages, as well as a decreased spacing between messages sent shortly after another.",
            2,
            GROUP_ID,
            18 + base_minute
        );
        let msg_reply = msg!(
            self,
            "Updating now! The UI changes already look good in the screenshots.",
            1,
            GROUP_ID,
            20 + base_minute
        );
        msg_reply
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .set_quote(&msg_replied.clone().downcast::<TextMessage>().unwrap());
        msg_replied
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react(&ReactionMessage::from_reaction(
                &self.dummy_contacts().await[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🎉🚀".to_string()),
                    remove: Some(false),
                    target_author_aci: None,
                    target_author_aci_binary: None,
                    target_sent_timestamp: None,
                },
            ));

        let msg_screenshot = msg!(self, "", 2, GROUP_ID, 19 + base_minute);
        let screenshot_file = gtk::gio::File::for_uri("resource:///icon.svg");
        let attachment = crate::backend::Attachment::from_file(screenshot_file, self);
        msg_screenshot
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .add_attachment(attachment)
            .await
            .expect("Failed to add attachment");

        vec![
            msg_replied,
            // msg_screenshot,
            msg_reply,
            msg!(
                self,
                "Indeed, the new release looks magnificent.",
                1,
                GROUP_ID,
                24 + base_minute
            ),
            msg!(self, "Glad you like it.", 2, GROUP_ID, 25 + base_minute),
            msg!(self, "YAY!", 0, GROUP_ID, 27 + base_minute),
            msg!(self, "Greetings", 2, 2, base_minute - 100),
            msg!(self, "It's a trap!", 3, 3, 1 + base_minute),
            msg!(self, "Hello, Mr. Anderson", 4, 4, 2 + base_minute),
        ]
    }

    #[cfg(feature = "screenshot")]
    async fn dummy_contacts(&self) -> Vec<Contact> {
        let mut result = vec![];
        for c in dummy_presage_contacts() {
            let contact = Contact::from_contact(c, self);
            let channel = Channel::from_contact_or_group(contact.clone(), &None, self).await;
            contact.set_channel(Some(&channel));
            contact.update_profile_name_and_avatar().await;
            result.push(contact);
        }
        result
    }

    #[cfg(feature = "screenshot")]
    async fn dummy_channels(&self) -> Vec<Channel> {
        let mut result = vec![];
        for con in self.dummy_contacts().await {
            result.push(Channel::from_contact_or_group(con, &None, self).await);
        }
        result.push(
            Channel::from_group(
                Group {
                    title: "Mobile Linux Group".to_string(),
                    avatar: "".to_string(),
                    disappearing_messages_timer: None,
                    access_control: None,
                    revision: 0,
                    members: dummy_presage_contacts()
                        .into_iter()
                        .map(|c| presage::model::groups::Member {
                            aci: c.uuid.into(),
                            role: libsignal_service::groups_v2::Role::Unknown,
                            profile_key: ProfileKey {
                                bytes: c.profile_key.try_into().unwrap_or_default(),
                            },
                            joined_at_revision: 0,
                        })
                        .collect(),
                    pending_members: vec![],
                    requesting_members: vec![],
                    invite_link_password: vec![],
                    description: None,
                },
                &GroupContextV2 {
                    master_key: Some(vec![2; 32]),
                    revision: None,
                    group_change: None,
                },
                self,
            )
            .await,
        );
        result
    }

    #[cfg(feature = "screenshot")]
    pub async fn init_channels(&self) {
        for channel in self.dummy_channels().await {
            self.emit_by_name::<()>("channel", &[&channel]);
            let mut channels = self.imp().channels.borrow_mut();
            channel.initialize_avatar().await;
            channels.insert(channel.thread(), channel);
        }
    }
}
