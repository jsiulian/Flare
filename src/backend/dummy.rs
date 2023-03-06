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

const GROUP_ID: usize = 16;

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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(0)),
                phonenumber: None,
                relay: None,
            },
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(1)),
                phonenumber: None,
                relay: None,
            },
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(2)),
                phonenumber: None,
                relay: None,
            },
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(3)),
                phonenumber: None,
                relay: None,
            },
            name: "Anakin Skywalker".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(4)),
                phonenumber: None,
                relay: None,
            },
            name: "Terminator".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(5)),
                phonenumber: None,
                relay: None,
            },
            name: "Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(6)),
                phonenumber: None,
                relay: None,
            },
            name: "Better Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(7)),
                phonenumber: None,
                relay: None,
            },
            name: "Best Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(8)),
                phonenumber: None,
                relay: None,
            },
            name: "Bestester Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(9)),
                phonenumber: None,
                relay: None,
            },
            name: "Ultra Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(10)),
                phonenumber: None,
                relay: None,
            },
            name: "Omega Friend".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(11)),
                phonenumber: None,
                relay: None,
            },
            name: "That guy again".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(12)),
                phonenumber: None,
                relay: None,
            },
            name: "Enemy".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(13)),
                phonenumber: None,
                relay: None,
            },
            name: "Rick".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(14)),
                phonenumber: None,
                relay: None,
            },
            name: "Microsoft Support".to_string(),
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
            address: presage::prelude::ServiceAddress {
                uuid: Some(Uuid::from_u128(15)),
                phonenumber: None,
                relay: None,
            },
            name: "Who is this?".to_string(),
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
        // let msg_replied = msg!(self, "Sounds interesting, can you tell me more?", 0, 2 + base_minute);
        // let msg_reply = msg!(self, "Additionally, replying and reacting to messages are also be possible", 1, 5 + base_minute);
        // msg_reply.set_quote(msg_replied.clone());
        // msg_reply.react("👍");
        //
        // let msg_screenshot = msg!(self, "", 1, 4 + base_minute);
        // let screenshot_file = gio::File::for_uri("resource:///icon.png");
        // let attachment = crate::backend::Attachment::from_file(screenshot_file, self);
        // msg_screenshot.add_attachment(attachment).await.expect("Failed to add attachment");
        vec![
            msg!(self, "I use Arch btw", 1, 1, 0 + base_minute),
            msg!(self, "Did you know Flare can also be used on mobile devices?", 2, GROUP_ID, 2 + base_minute),
            msg!(self, "WHAT", 0, GROUP_ID, 4 + base_minute),
            msg!(self, "Yes, you can just use it on any of your favorite mobile linux devices.", 2, GROUP_ID, 10 + base_minute),
            msg!(self, "What is Flare?", 1, GROUP_ID, 15 + base_minute),
            msg!(self, "It is an unofficial Signal client.", 2, GROUP_ID, 16 + base_minute),
            msg!(self, "I don't think I need to go over all of the features again, look at the previous screenshots for more details.", 2, GROUP_ID, 17 + base_minute),
            msg!(self, "Looks interesting. Might I will give it a shot on my PinePhone where I am running Arch btw.", 1, GROUP_ID, 20 + base_minute),
            msg!(self, "Could you please stop? We all know that you are using Arch", 0, GROUP_ID, 21 + base_minute),
            msg!(self, "But as an Arch User (btw), it is my holy duty to inform you that I am using Arch (btw) at least every second message.", 1, GROUP_ID, 22 + base_minute),
            msg!(self, "Could we please continue this discussion in the next screenshot? Due to me also making a screenshot in a mobile formfactor, there is not that much space left.", 2, GROUP_ID, 23 + base_minute),
            call_msg!(self, PreCallMessage {
                offer: Some(Offer::default()),
                ..Default::default()
            }, 2, 21 + base_minute),
            call_msg!(self, PreCallMessage {
                hangup: Some(Hangup::default()),
                ..Default::default()
            }, 2, 22 + base_minute),
            msg!(self, "I don't like sand", 3, 3, base_minute + 3),
            msg!(self, "I'll be back", 4, 4, base_minute + 10),

            msg!(self, "Here could be your meme", 5, 5, base_minute - 1),
            msg!(self, "Here could be your meme", 6, 6, base_minute - 2),
            msg!(self, "Here could be your meme", 7, 7, base_minute - 3),
            msg!(self, "Here could be your meme", 8, 8, base_minute - 4),
            msg!(self, "Here could be your meme", 9, 9, base_minute - 5),
            msg!(self, "Here could be your meme", 10, 10, base_minute - 6),
            msg!(self, "You again?", 0, 11, base_minute - 7),
            msg!(self, "Here could be your meme", 12, 12, base_minute - 8),
            msg!(self, "Imagine a Rick-Roll here.", 13, 13, base_minute - 9),
            msg!(self, "Hello, you have 10 virus. Please click link.", 14, 14, base_minute - 9),
            msg!(self, "Who is this?", 0, 15, base_minute - 50),
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
                    version: 0,
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
