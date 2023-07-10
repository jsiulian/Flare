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
            name: "Arch Linux Mobile User".to_string(),
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
            name: "Richard".to_string(),
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
            name: "Cowboy".to_string(),
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
            name: "Call Center".to_string(),
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
    pub fn profile_name(&self) -> String {
        "You".to_string()
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
            "We've got another version of Flare. Version 0.9.0 brings massive improvements to the UI, mainly in regards to the message list. This list got completely overhauled for a better UI experience and now (among other changes) supports being styled by GNOME accent colors and a reworked message popover (which now get opened by right-click or long-press on the message). Flare got also ported to the Blueprint markup language to make development easier and to GTK 4.10 to keep up-to-date with the latest standards. Finally, Flare now supports playback of voice messages, which should make Flare viable for people having that one annoying friend always sending voice messages. For most of those changes, I want to thank @Marc0x (yes, we also now have mentions, but I am too lazy to show them in this screenshot).",
            2,
            GROUP_ID,
            18 + base_minute
        );
        let msg_reply = msg!(
            self,
            "Thats awesome! Probably, as always, there were many bug fixes :)",
            0,
            GROUP_ID,
            19 + base_minute
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
                &self.dummy_contacts()[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🎉🚀".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ));
        msg_reply.clone().downcast::<TextMessage>().unwrap().react(
            &ReactionMessage::from_reaction(
                &self.dummy_contacts()[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("😊".to_string()),
                    remove: Some(false),
                    target_author_uuid: None,
                    target_sent_timestamp: None,
                },
            ),
        );

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
            msg!(
                self,
                "And finally voice messages. I have long awaited this moment.",
                0,
                GROUP_ID,
                20 + base_minute
            ),
            msg!(
                self,
                "Great to see more contributions to Flare.",
                1,
                GROUP_ID,
                23 + base_minute
            ),
            msg!(
                self,
                "Indeed. I am terrible at UI (if you don't believe me, just look at the first screenshot of Flare - if you dare), so contributions to that are very welcome.",
                2,
                GROUP_ID,
                24 + base_minute
            ),
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
            msg!(
                self,
                "I'd like to interject for a moment. What you call",
                3,
                3,
                1 + base_minute
            ),
            msg!(self, "First time?", 4, 4, 2 + base_minute),
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
