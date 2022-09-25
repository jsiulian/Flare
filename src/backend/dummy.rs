use std::path::Path;

use gdk::{prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};
use presage::prelude::*;
use libsignal_service::sender::AttachmentUploadError;
use glib::DateTime;

use super::{Channel, Contact, Message};

macro_rules! msg {
    ($s:expr, $m:expr, $i:expr, $j:expr, $t:expr) => {
        Message::pub_from_text_channel_sender_timestamp(
            $m,
            $s.dummy_channels().await[$j].clone(),
            $s.dummy_contacts()[$i].clone(),
            $t * 1000 * 60,
            $s,
        )
    };
    ($s:expr, $m:expr, $i:expr, $t:expr) => {
        msg!($s, $m, $i, 1, $t)
    };
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
                uuid: Some(Uuid::from_u128(2)),
                phonenumber: None,
                relay: None,
            },
            name: "Officer".to_string(),
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
            name: "That WOW-Guy".to_string(),
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
    pub async fn init<P: AsRef<Path>>(
        &self,
        _p: &P,
    ) -> Result<(), crate::ApplicationError> {
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
            if let Some(stored_channel) = channels.get(
                &msg.channel()
                    .expect("Screenshot message to have channel")
                    .internal_hash(),
            ) {
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
        let base_time = DateTime::from_utc(now.year(), now.month(), now.day_of_month(), 11, 0, 0.0).expect("Base time to be expressable as DateTime");
        let base_minute: u64 = (base_time.to_unix() / 60).try_into().unwrap();
        let msg_replied = msg!(self, "Sounds interesting, can you tell me more?", 0, 2 + base_minute);
        let msg_reply = msg!(self, "Additionally, replying and reacting to messages are also be possible", 1, 5 + base_minute);
        msg_reply.set_quote(msg_replied.clone());
        msg_reply.react("👍");

        let msg_screenshot = msg!(self, "", 1, 4 + base_minute);
        let screenshot_file = gio::File::for_uri("resource:///icon.png");
        let attachment = crate::backend::Attachment::from_file(screenshot_file, self);
        msg_screenshot.add_attachment(attachment).await.expect("Failed to add attachment");

        vec![
            msg!(self, "Hello", 0, 0 + base_minute),
            msg!(self, "Hello, may I present to you my application Flare?", 1, 1 + base_minute),
            msg!(self, "It is an unofficial Signal client.", 1, 1 + base_minute),
            msg_replied,
            msg!(self, "Well, it is a pretty simple application. As you can clearly see, it supports sending and receiving messages.", 1, 3 + base_minute),
            msg!(self, "Pictures and other attachments can also be sent:", 1, 4 + base_minute),
            msg_screenshot,
            msg_reply,
            msg!(self, "Looks pretty nice, where can I try it?", 0, 6 + base_minute),
            msg!(self, "Just head over to Flathub and download it.", 1, 8 + base_minute),
            msg!(self, "As the free space for this screenshot is almost over, I have just one more question: ", 1, 8 + base_minute),
            msg!(self, "Why are you still reading this? In the time it took you to read it, you could have already downloaded it and set it up.", 1, 8 + base_minute),
            msg!(self, "Thats gotta be the best application I've ever seen", 2, 2, base_minute - 100),
            msg!(self, "WOW", 3, 3, base_minute - 200),
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
