use std::cell::OnceCell;
use std::path::Path;

use gdk::{prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};
use gtk::glib::{BoxedAnyObject, Cast, DateTime};
use libsignal_service::content::CallMessage as PreCallMessage;
use libsignal_service::models::Contact as LContact;
use libsignal_service::prelude::AttachmentPointer;
use libsignal_service::prelude::Content;
use libsignal_service::prelude::Uuid;
use libsignal_service::proto::call_message::Hangup;
use libsignal_service::proto::call_message::Offer;
use libsignal_service::proto::data_message::Reaction;
use libsignal_service::proto::GroupContextV2;
use libsignal_service::sender::AttachmentSpec;
use libsignal_service::{groups_v2::Group, sender::AttachmentUploadError};
use presage::store::Thread;

use super::{
    message::{CallMessage, Message, MessageExt, ReactionMessage, TextMessage},
    Channel, Contact,
};
use crate::{backend::SetupResult, error::ApplicationError};

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

pub fn dummy_presage_contacts() -> Vec<LContact> {
    vec![
        LContact {
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
        LContact {
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
        LContact {
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
        LContact {
            uuid: Uuid::from_u128(3),
            phone_number: None,
            name: "T'Challa".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        LContact {
            uuid: Uuid::from_u128(4),
            phone_number: None,
            name: "Toad".to_string(),
            color: None,
            verified: Default::default(),
            profile_key: vec![],
            blocked: false,
            expire_timer: 0,
            inbox_position: 0,
            archived: false,
            avatar: None,
        },
        LContact {
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
            "Another new version of Flare. Version 0.10.0 brings many new UI improvements, bug fixes and also features. The biggest feature this release it the support for blurhashes for images and videos, this makes loading images and videos a lot smoother and visually appealing. Furthermore, one can now delete all the messages of a single chat. But this was not all, since the last screenshot, there has been numerous of changes, features and improvements.",
            2,
            GROUP_ID,
            18 + base_minute
        );
        let msg_reply = msg!(
            self,
            "Nice to see continued improvement with Flare. I also noticed that the URL of the projects repository has changed. Whats up with that?",
            0,
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
                &self.dummy_contacts()[0],
                &self.dummy_channels().await[GROUP_ID],
                26 + base_minute,
                &self,
                Reaction {
                    emoji: Some("🎉🚀".to_string()),
                    remove: Some(false),
                    target_author_aci: None,
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
                    target_author_aci: None,
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
                "Yes, we have now decided to put all of my applications that are targetted towards Linux Mobile in a separate GitLab group, called 'Schmiddi on Mobile' (I know, it's a bit egoistic. But we could not find a better name). There, you can also find Pipeline for playing YouTube and Peertube videos, and DieBahn (pending a rename) for all your public transport needs.",
                2,
                GROUP_ID,
                24 + base_minute
            ),
            msg!(
                self,
                "Oh, and we also have a Mastodon account by the way. It can be found at @schmiddionmobile@fosstodon.org.",
                2,
                GROUP_ID,
                25 + base_minute
            ),
            msg!(
                self,
                "Yes, I already noticed that Mastodon account. Already following.",
                0,
                GROUP_ID,
                26 + base_minute
            ),
            msg!(
                self,
                "I did not know you also had other apps. Will need to check them out.",
                1,
                GROUP_ID,
                27 + base_minute
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
                "We don't do that here",
                3,
                3,
                1 + base_minute
            ),
            msg!(self, "Thank you! But our princess is in another castle", 4, 4, 2 + base_minute),
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
