use std::path::Path;

use gdk::{prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};
use gtk::glib::{Cast, DateTime};
use libsignal_service::{groups_v2::Group, sender::AttachmentUploadError};
use presage::prelude::{
    content::{AttachmentPointer, CallMessage as PreCallMessage},
    proto::{
        call_message::{Hangup, Offer},
        data_message::Reaction,
    },
    *,
};
use presage::Thread;

use super::{
    message::{CallMessage, Message, MessageExt, ReactionMessage, TextMessage},
    Channel, Contact,
};
use crate::error::ApplicationError;

const GROUP_ID: usize = 6;
type PresageError = presage::Error<presage_store_sled::SledStoreError>;

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
            name: "Postmarket OS User".to_string(),
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
            name: "Unnamed old man".to_string(),
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
            name: "Palpatine".to_string(),
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
    pub async fn setup_receive_message_loop(&self) -> Result<(), PresageError> {
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
            r#"Flare 0.8.0 was released with an improved GUI for the message list. The list is now "smart" and automatically loads messages if more are required. Long live Gtk ListView (and a few things I did)!"#,
            2,
            GROUP_ID,
            18 + base_minute
        );
        let msg_reply = msg!(
            self,
            "RIP \"Load More\"-button. Juni 2022 - May 2023. You won't be missed.",
            0,
            GROUP_ID,
            25 + base_minute
        );
        msg_reply
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .set_quote(&msg_replied.clone().downcast::<TextMessage>().unwrap());
        msg_reply.clone().downcast::<TextMessage>().unwrap().react(
            &ReactionMessage::from_reaction(
                &self.dummy_contacts()[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🤣️".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ),
        );

        let msg_reacted = msg!(
            self,
            "Oh, and as always there were many fixes in this version, including a few regarding emojis.",
            2,
            GROUP_ID,
            27 + base_minute
        );
        msg_reacted
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react(&ReactionMessage::from_reaction(
                &self.dummy_contacts()[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("👨‍👨‍👧‍👧️".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ));
        msg_reacted
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react(&ReactionMessage::from_reaction(
                &self.dummy_contacts()[1],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🏴‍☠️️".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ));
        msg_reacted
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react(&ReactionMessage::from_reaction(
                &self.dummy_contacts()[2],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🤌🏼️".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ));

        let msg_reacted2 = msg!(self, "Wow, Flare is now almost one year old. It certainly has come far in that time. Who bakes the cake for the birthday party?", 1, GROUP_ID, 35 + base_minute);

        msg_reacted2
            .clone()
            .downcast::<TextMessage>()
            .unwrap()
            .react(&ReactionMessage::from_reaction(
                &self.dummy_contacts()[2],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🎂".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ));

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
            msg_screenshot,
            msg_reply,
            msg_reacted,
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
            msg_reacted2,
            msg!(
                self,
                "It has indeed improved a lot. Let's hope for an equally bright future.",
                0,
                GROUP_ID,
                36 + base_minute
            ),
            msg!(
                self,
                "Let's hope I don't have to prepare a speech 😃️",
                2,
                GROUP_ID,
                37 + base_minute
            ),
            msg!(
                self,
                "Anyway, as there is still space left in this screenshots, here are a few shameless plugs:
- You can get Flare on Flathub (with a new design 😍️).
- It's also available on Alpine Edge (and therefore Postmarket OS Edge).
- It's also available in the AUR for Arch users (punish your PinePhone by compiling this).
- You can also get involved in Flare by translating it over at Weblate.
- Feel free to join the Matrix room and talk a bit.",
                2,
                GROUP_ID,
                38 + base_minute
            ),
            msg!(
                self,
                "It's dangerous to go alone! Take this.",
                3,
                3,
                1 + base_minute
            ),
            msg!(self, "Fly you fools", 4, 4, 2 + base_minute),
            msg!(self, "Do it!", 5, 5, 3 + base_minute),
            msg!(
                self,
                "Flare is also packaged in PMOS!",
                1,
                1,
                3 + base_minute
            ),
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
