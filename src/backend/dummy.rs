use std::path::Path;

use gdk::{prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};
use gtk::glib::{Cast, DateTime};
use libsignal_service::{groups_v2::Group, sender::AttachmentUploadError};
use presage::prelude::{
    content::{AttachmentPointer, CallMessage as PreCallMessage},
    proto::call_message::{Hangup, Offer},
    *,
};

use super::{
    message::{CallMessage, Message, MessageExt, TextMessage},
    Channel, Contact,
};

const GROUP_ID: usize = 6;

macro_rules! msg {
    ($s:expr, $m:expr, $i:expr, $j:expr, $t:expr) => {
        TextMessage::pub_from_text_channel_sender_timestamp(
            $m,
            $s.dummy_channels().await[$j].clone(),
            $s.dummy_contacts()[$i].clone(),
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
        let c = $s.dummy_contacts()[$i].clone();
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

pub fn dummy_presage_contacts() -> Vec<presage::prelude::Contact> {
    vec![
        presage::prelude::Contact {
            uuid: Uuid::from_u128(0),
            phone_number: None,
            name: "".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        presage::prelude::Contact {
            uuid: Uuid::from_u128(1),
            phone_number: None,
            name: "Arch Linux User".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        presage::prelude::Contact {
            uuid: Uuid::from_u128(2),
            phone_number: None,
            name: "Developer".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        presage::prelude::Contact {
            uuid: Uuid::from_u128(3),
            phone_number: None,
            name: "Johnny".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        presage::prelude::Contact {
            uuid: Uuid::from_u128(4),
            phone_number: None,
            name: "Gandalf".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        presage::prelude::Contact {
            uuid: Uuid::from_u128(5),
            phone_number: None,
            name: "Vader".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
    ]
}

impl super::Manager {
    #[cfg(feature = "screenshot")]
    pub async fn init<P: AsRef<Path>>(&self, _p: &P) -> Result<(), crate::ApplicationError> {
        log::trace!("Init manager for screenshots");
        self.init_channels().await;
        self.setup_receive_message_loop().await?;
        Ok(())
    }

    #[cfg(feature = "screenshot")]
    pub async fn setup_receive_message_loop(&self) -> Result<(), presage::Error> {
        log::trace!("Setup receive loop for screenshots");
        let channels = self.imp().channels.borrow();

        for msg in self.dummy_messages().await {
            self.emit_by_name::<()>("message", &[&msg]);
            if let Some(stored_channel) = channels.get(&msg.channel().internal_hash()) {
                log::debug!("Message from a already existing channel");
                let _ = stored_channel.new_message(msg).await;
            }
        }
        Ok(())
    }

    #[cfg(feature = "screenshot")]
    pub(super) fn uuid(&self) -> Uuid {
        Uuid::nil()
    }

    #[cfg(feature = "screenshot")]
    pub async fn upload_attachments(
        &self,
        attachments: Vec<(AttachmentSpec, Vec<u8>)>,
    ) -> Result<Vec<Result<AttachmentPointer, AttachmentUploadError>>, presage::Error> {
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
            "Flare has now got a fresh new UI.",
            2,
            GROUP_ID,
            18 + base_minute
        );
        let msg_reply = msg!(
            self,
            "Looks really good. Nice work.",
            0,
            GROUP_ID,
            25 + base_minute
        );
        msg_reply
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .set_quote(&msg_replied.clone().downcast::<TextMessage>().unwrap());
        msg_reply
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react("👍");

        let msg_screenshot = msg!(self, "", 2, GROUP_ID, 19 + base_minute);
        let screenshot_file = gtk::gio::File::for_uri("resource:///icon.png");
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
            msg_reply,
            msg_screenshot,
            msg!(self, "Nice indeed.", 1, GROUP_ID, 27 + base_minute),
            call_msg!(
                self,
                PreCallMessage {
                    offer: Some(Offer::default()),
                    ..Default::default()
                },
                2,
                base_minute - 100
            ),
            call_msg!(
                self,
                PreCallMessage {
                    hangup: Some(Hangup::default()),
                    ..Default::default()
                },
                2,
                base_minute - 99
            ),
            msg!(self, "Thats not all. In addition to that, Flare now support profile names (no more phone-number contacts). Additionally, Flare now integrates with feedbackd.", 2, GROUP_ID, 30 + base_minute),
            msg!(self, "And as always, of course, a few bug fixes, overall improvements and dependency updates. For all changes, see the changelog.", 2, GROUP_ID, 32 + base_minute),
            msg!(self, "Due to one dependency update, you will need to relink your device after the update.", 2, GROUP_ID, 33 + base_minute),
            msg!(self, "Thanks for the warning", 1, GROUP_ID, 35 + base_minute),
            msg!(self, "Here's Johnny", 3, 3, 1 + base_minute),
            msg!(self, "Flee you fools", 4, 4, 2 + base_minute),
            msg!(self, "I am your father", 5, 5, 3 + base_minute),
            msg!(self, "Don't you dare say it!", 0, 1, 3 + base_minute),
        ]
    }

    #[cfg(feature = "screenshot")]
    fn dummy_contacts(&self) -> Vec<Contact> {
        dummy_presage_contacts()
            .into_iter()
            .map(|c| Contact::from_contact(c, self))
            .collect()
    }

    #[cfg(feature = "screenshot")]
    async fn dummy_channels(&self) -> Vec<Channel> {
        let mut result = vec![];
        for con in self.dummy_contacts() {
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
                    members: vec![],
                    pending_members: vec![],
                    requesting_members: vec![],
                    invite_link_password: vec![],
                    description: None,
                },
                &GroupContextV2 {
                    master_key: Some(vec![2]),
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
            channels.insert(channel.internal_hash(), channel);
        }
    }
}
