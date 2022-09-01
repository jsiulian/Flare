use super::{Channel, Contact, Message};
use gdk::prelude::ObjectExt;
use gdk::subclass::prelude::ObjectSubclassIsExt;
use presage::prelude::Uuid;
use std::path::Path;
use presage::prelude::{proto::AttachmentPointer, AttachmentSpec};
use libsignal_service::sender::AttachmentUploadError;

macro_rules! msg {
    ($s:expr, $m:expr, $i:expr, $j:expr) => {
        Message::from_text_channel_sender(
            $m,
            $s.dummy_channels().await[$j].clone(),
            $s.dummy_contacts()[$i].clone(),
            $s
        )
    };
    ($s:expr, $m:expr, $i:expr) => {
        msg!($s, $m, $i, 1)
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
            name: "Imaginary Friend".to_string(),
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
            name: "Rick Astley".to_string(),
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
            name: "Obi-Wan Kenobi".to_string(),
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
                let _ = stored_channel.new_message(msg);
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
        let msg_replied = msg!(self, "And what can that Flare-thing actually do?", 1);
        let msg_reply = msg!(self, "Additionally, replying and reacting to messages should also be possible", 0);
        msg_reply.set_quote(msg_replied.clone());
        msg_reply.react("👍");

        let msg_screenshot = msg!(self, "", 0);
        let screenshot_file = gio::File::for_uri("resource:///icon.png");
        let attachment = crate::backend::Attachment::from_file(screenshot_file, self);
        msg_screenshot.add_attachment(attachment).await.expect("Failed to add attachment");

        vec![
            msg!(self, "Hello", 1),
            msg!(self, "Hi, how are you", 0),
            msg!(self, "Pretty ok, but I somehow feel a little imaginary", 1),
            msg!(self, "What do you mean?", 0),
            msg!(self, "Like I was hard-coded in some application", 1),
            msg!(self, "I know it is hard to understand", 1),
            msg!(self, "I think I just exist exist to provide example data for some application", 1),
            msg!(self, "I can't believe you are a Flarer", 0),
            msg!(self, "What is a Flarer", 1),
            msg!(self, "You have seriously not heared about Flare before?", 0),
            msg_screenshot,
            msg!(self, "Some people (the Flarers) believe that they are just some example data for a Signal client named Flare", 0),
            msg!(self, "That has to be the weirdest conspiracy theory I have ever heared of", 1),
            msg_replied,
            msg!(self, "It is told to be a very simple GTK based signal client", 0),
            msg!(self, "As told, it only supports sending and receiving messages to contacts or groups", 0),
            msg_reply,
            msg!(self, "And some even think more features might come in the future", 0),
            msg!(self, "Wow. But does anybody actually believe in this?", 1),
            msg!(self, "Never going to give you up, never going to let you down, never going to ", 2, 2),
            msg!(self, "Hello there", 3, 3),
            msg!(self, "I doubt it.", 0),
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
