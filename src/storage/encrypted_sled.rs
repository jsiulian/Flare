use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use encrypted_sled::IVec;
use libsignal_service::{
    content::Content,
    models::Contact,
    prelude::{
        protocol::{
            Context, Direction, IdentityKey, IdentityKeyPair, IdentityKeyStore, PreKeyRecord,
            PreKeyStore, ProtocolAddress, SessionRecord, SessionStore, SessionStoreExt,
            SignalProtocolError, SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore,
        },
        ProtobufMessage, Uuid,
    },
};
use libsignal_service::{prelude::protocol::PreKeyId, push_service::DEFAULT_DEVICE_ID};
use log::{debug, trace, warn};

use presage::{
    ContactsStore, ContentProto, Error, MessageStore, Registered, StateStore, Store, Thread,
};

const SLED_KEY_REGISTRATION: &str = "registration";
const SLED_KEY_CONTACTS: &str = "contacts";
const SLED_KEY_GROUPS: &str = "groups";

const SLED_TREE_SESSIONS: &str = "sessions";
const SLED_TREE_THREAD_PREFIX: &str = "thread";

#[derive(Debug, Clone)]
pub struct EncryptedSledStore<E> {
    db: Arc<RwLock<encrypted_sled::Db<E>>>,
}

impl<E: encrypted_sled::Encryption + 'static> EncryptedSledStore<E> {
    pub fn new(path: impl Into<PathBuf>, encryption: E) -> Result<Self, Error> {
        Ok(EncryptedSledStore {
            db: Arc::new(RwLock::new(encrypted_sled::open(path.into(), encryption)?)),
        })
    }

    pub fn clear(&self) -> Result<(), Error> {
        log::trace!("Clearing config store");
        let db = self.db.read().expect("poisoned mutex");
        db.clear()?;
        db.flush()?;
        let tree_sessions = db.open_tree(SLED_TREE_SESSIONS)?;
        tree_sessions.clear()?;
        tree_sessions.flush()?;
        let tree_contacts = db.open_tree(SLED_KEY_CONTACTS)?;
        tree_contacts.clear()?;
        tree_contacts.flush()?;
        drop(db);
        Ok(())
    }

    pub fn clear_messages(&self) -> Result<(), Error> {
        let db = self.db.read().expect("poisoned mutex");
        for name in db.tree_names().expect("Failed to get tree names") {
            if name
                .as_ref()
                .starts_with(SLED_TREE_THREAD_PREFIX.as_bytes())
            {
                let tree_thread = db.open_tree(&name)?;
                tree_thread.clear()?;
                tree_thread.flush()?;
            }
        }
        Ok(())
    }

    pub fn get<K>(&self, key: K) -> Result<Option<IVec>, Error>
    where
        K: AsRef<str>,
    {
        trace!("get {}", key.as_ref());
        Ok(self.db.read().expect("poisoned mutex").get(key.as_ref())?)
    }

    fn get_u32<S>(&self, key: S) -> Result<Option<u32>, Error>
    where
        S: AsRef<str>,
    {
        trace!("getting u32 {}", key.as_ref());
        Ok(self.get(key.as_ref())?.map(|data| {
            let mut a: [u8; 4] = Default::default();
            a.copy_from_slice(&data);
            u32::from_le_bytes(a)
        }))
    }

    fn insert<K, V>(&self, key: K, value: V) -> Result<(), Error>
    where
        K: AsRef<str>,
        IVec: From<V>,
    {
        trace!("inserting {}", key.as_ref());
        let _ = self
            .db
            .write()
            .expect("poisoned mutex")
            .insert(key.as_ref(), value)?;
        Ok(())
    }

    fn merge<K, V>(&self, key: K, value: V) -> Result<(), Error>
    where
        K: AsRef<str>,
        IVec: From<V>,
    {
        trace!("inserting {}", key.as_ref());
        let db = self.db.write().expect("poisoned mutex");
        db.set_merge_operator(prefix_merge);
        db.merge(key.as_ref(), IVec::from(value))?;
        Ok(())
    }

    pub fn save_group(&self, key: &[u8]) -> Result<(), Error> {
        self.merge(SLED_KEY_GROUPS, key)
    }

    pub fn get_groups(&self) -> Result<Vec<[u8; 32]>, Error> {
        Ok(self
            .get(SLED_KEY_GROUPS)?
            .as_ref()
            .map(<sled::IVec as std::borrow::Borrow<[u8]>>::borrow)
            .unwrap_or_default()
            .chunks_exact(32)
            .map(|buf| buf.try_into().expect("buf to have length 32"))
            .collect())
    }

    fn insert_u32<S>(&self, key: S, value: u32) -> Result<(), Error>
    where
        S: AsRef<str>,
    {
        trace!("inserting u32 {}", key.as_ref());
        self.db
            .write()
            .expect("poisoned mutex")
            .insert(key.as_ref(), &value.to_le_bytes())?;
        Ok(())
    }

    fn remove<S>(&self, key: S) -> Result<(), Error>
    where
        S: AsRef<str>,
    {
        trace!("removing {} from db", key.as_ref());
        self.db
            .write()
            .expect("poisoned mutex")
            .remove(key.as_ref())?;
        Ok(())
    }

    fn prekey_key(&self, id: PreKeyId) -> String {
        format!("prekey-{:09}", id)
    }

    fn signed_prekey_key(&self, id: SignedPreKeyId) -> String {
        format!("signed-prekey-{:09}", id)
    }
    fn session_key(&self, addr: &ProtocolAddress) -> String {
        format!("session-{}", addr)
    }

    fn session_prefix(&self, name: &str) -> String {
        format!("session-{}.", name)
    }

    fn identity_key(&self, addr: &ProtocolAddress) -> String {
        format!("identity-remote-{}", addr)
    }
}

impl<E: encrypted_sled::Encryption> StateStore<Registered> for EncryptedSledStore<E> {
    fn load_state(&self) -> Result<Registered, Error> {
        let db = self.db.read().expect("poisoned mutex");
        let data = db
            .get(SLED_KEY_REGISTRATION)?
            .ok_or(Error::NotYetRegisteredError)?;
        serde_json::from_slice(&data).map_err(Error::from)
    }

    fn save_state(&mut self, state: &Registered) -> Result<(), Error> {
        let db = self.db.write().expect("poisoned mutex");
        db.clear()?;
        db.insert(SLED_KEY_REGISTRATION, serde_json::to_vec(state)?)?;
        Ok(())
    }
}

impl<E: encrypted_sled::Encryption + 'static> Store for EncryptedSledStore<E>
where
    E: std::marker::Send + std::marker::Sync + std::clone::Clone,
{
    fn pre_keys_offset_id(&self) -> Result<u32, Error> {
        Ok(self.get_u32("pre_keys_offset_id")?.unwrap_or(0))
    }

    fn set_pre_keys_offset_id(&mut self, id: u32) -> Result<(), Error> {
        self.insert_u32("pre_keys_offset_id", id)
    }

    fn next_signed_pre_key_id(&self) -> Result<u32, Error> {
        Ok(self.get_u32("next_signed_pre_key_id")?.unwrap_or(0))
    }

    fn set_next_signed_pre_key_id(&mut self, id: u32) -> Result<(), Error> {
        self.insert_u32("next_signed_pre_key_id", id)
    }
}

impl<E: encrypted_sled::Encryption> ContactsStore for EncryptedSledStore<E> {
    fn save_contacts(&mut self, contacts: impl Iterator<Item = Contact>) -> Result<(), Error> {
        let tree = self
            .db
            .write()
            .expect("poisoned mutex")
            .open_tree(SLED_KEY_CONTACTS)?;
        for contact in contacts {
            if let Some(uuid) = contact.address.uuid {
                tree.insert(uuid.to_string(), serde_json::to_vec(&contact)?)?;
            } else {
                warn!("skipping contact {:?} without uuid", contact);
            }
        }
        tree.flush()?;
        debug!("saved contacts");
        Ok(())
    }

    fn contacts(&self) -> Result<Vec<Contact>, Error> {
        Ok(self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(SLED_KEY_CONTACTS)?
            .iter()
            .filter_map(Result::ok)
            .filter_map(|(_key, buf)| serde_json::from_slice(&buf).ok())
            .collect())
    }

    fn contact_by_id(&self, id: Uuid) -> Result<Option<Contact>, Error> {
        let db = self.db.read().expect("poisoned mutex");
        Ok(
            if let Some(buf) = db.open_tree(SLED_KEY_CONTACTS)?.get(id.to_string())? {
                let contact = serde_json::from_slice(&buf)?;
                Some(contact)
            } else {
                None
            },
        )
    }
}

#[async_trait(?Send)]
impl<E: encrypted_sled::Encryption + 'static> PreKeyStore for EncryptedSledStore<E> {
    async fn get_pre_key(
        &self,
        prekey_id: PreKeyId,
        _ctx: Context,
    ) -> Result<PreKeyRecord, SignalProtocolError> {
        let buf = self
            .get(self.prekey_key(prekey_id))
            .unwrap()
            .ok_or(SignalProtocolError::InvalidPreKeyId)?;
        PreKeyRecord::deserialize(&buf)
    }

    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
        _ctx: Context,
    ) -> Result<(), SignalProtocolError> {
        self.insert(self.prekey_key(prekey_id), record.serialize()?)
            .expect("failed to store pre-key");
        Ok(())
    }

    async fn remove_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        _ctx: Context,
    ) -> Result<(), SignalProtocolError> {
        self.remove(self.prekey_key(prekey_id))
            .expect("failed to remove pre-key");
        Ok(())
    }
}

#[async_trait(?Send)]
impl<E: encrypted_sled::Encryption + 'static> SignedPreKeyStore for EncryptedSledStore<E> {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
        _ctx: Context,
    ) -> Result<SignedPreKeyRecord, SignalProtocolError> {
        let buf = self
            .get(self.signed_prekey_key(signed_prekey_id))
            .unwrap()
            .ok_or(SignalProtocolError::InvalidSignedPreKeyId)?;
        SignedPreKeyRecord::deserialize(&buf)
    }

    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
        _ctx: Context,
    ) -> Result<(), SignalProtocolError> {
        self.insert(
            self.signed_prekey_key(signed_prekey_id),
            record.serialize()?,
        )
        .map_err(|e| {
            log::error!("sled error: {}", e);
            SignalProtocolError::InvalidState("save_signed_pre_key", "sled error".into())
        })
    }
}

#[async_trait(?Send)]
impl<E: encrypted_sled::Encryption + 'static> SessionStore for EncryptedSledStore<E> {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
        _ctx: Context,
    ) -> Result<Option<SessionRecord>, SignalProtocolError> {
        let key = self.session_key(address);
        trace!("loading session from {}", key);

        let buf = self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(SLED_TREE_SESSIONS)
            .unwrap()
            .get(key)
            .unwrap();

        buf.map(|buf| SessionRecord::deserialize(&buf)).transpose()
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
        _ctx: Context,
    ) -> Result<(), SignalProtocolError> {
        let key = self.session_key(address);
        trace!("storing session for {:?} at {:?}", address, key);
        self.db
            .write()
            .expect("poisoned mutex")
            .open_tree(SLED_TREE_SESSIONS)
            .unwrap()
            .insert(key, record.serialize()?)
            .unwrap();
        Ok(())
    }
}

#[async_trait]
impl<E: encrypted_sled::Encryption + 'static> SessionStoreExt for EncryptedSledStore<E>
where
    E: std::marker::Send + std::marker::Sync + std::clone::Clone,
{
    async fn get_sub_device_sessions(&self, name: &str) -> Result<Vec<u32>, SignalProtocolError> {
        let session_prefix = self.session_prefix(name);
        log::info!("get_sub_device_sessions: session_prefix={}", session_prefix);
        let session_ids: Vec<u32> = self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(SLED_TREE_SESSIONS)
            .unwrap()
            .scan_prefix(&session_prefix)
            .expect("Decryption failed") // TODO: Better error handling
            .filter_map(|r| {
                let (key, _) = r.ok()?;
                let key_str = String::from_utf8_lossy(&key);
                let device_id = key_str.strip_prefix(&session_prefix)?;
                device_id.parse().ok()
            })
            .filter(|d| *d != DEFAULT_DEVICE_ID)
            .collect();
        Ok(session_ids)
    }

    async fn delete_session(&self, address: &ProtocolAddress) -> Result<(), SignalProtocolError> {
        let key = self.session_key(address);
        trace!("deleting session with key: {}", key);
        self.db
            .write()
            .expect("poisoned mutex")
            .open_tree(SLED_TREE_SESSIONS)
            .unwrap()
            .remove(key)
            .map_err(|_e| SignalProtocolError::SessionNotFound(address.clone()))?;
        Ok(())
    }

    async fn delete_all_sessions(&self, _name: &str) -> Result<usize, SignalProtocolError> {
        let tree = self
            .db
            .write()
            .expect("poisoned mutex")
            .open_tree(SLED_TREE_SESSIONS)
            .unwrap();
        let len = tree.len();
        tree.clear().map_err(|_e| {
            SignalProtocolError::InvalidSessionStructure("failed to delete all sessions")
        })?;
        Ok(len)
    }
}

#[async_trait(?Send)]
impl<E: encrypted_sled::Encryption + 'static> IdentityKeyStore for EncryptedSledStore<E> {
    async fn get_identity_key_pair(
        &self,
        _ctx: Context,
    ) -> Result<IdentityKeyPair, SignalProtocolError> {
        trace!("getting identity_key_pair");
        let state = self.load_state().map_err(|e| {
            SignalProtocolError::InvalidState("failed to load presage state", e.to_string())
        })?;
        Ok(IdentityKeyPair::new(
            IdentityKey::new(state.public_key()),
            state.private_key(),
        ))
    }

    async fn get_local_registration_id(&self, _ctx: Context) -> Result<u32, SignalProtocolError> {
        let state = self.load_state().map_err(|e| {
            SignalProtocolError::InvalidState("failed to load presage state", e.to_string())
        })?;
        Ok(state.registration_id())
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity_key: &IdentityKey,
        _ctx: Context,
    ) -> Result<bool, SignalProtocolError> {
        trace!("saving identity");
        self.insert(self.identity_key(address), identity_key.serialize())
            .map_err(|e| {
                log::error!("error saving identity for {:?}: {}", address, e);
                SignalProtocolError::InvalidState("save_identity", "failed to save identity".into())
            })?;
        trace!("saved identity");
        Ok(false)
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity_key: &IdentityKey,
        _direction: Direction,
        _ctx: Context,
    ) -> Result<bool, SignalProtocolError> {
        match self.get(self.identity_key(address)).map_err(|_| {
            SignalProtocolError::InvalidState(
                "is_trusted_identity",
                "failed to check if identity is trusted".into(),
            )
        })? {
            None => {
                // when we encounter a new identity, we trust it by default
                warn!("trusting new identity {:?}", address);
                Ok(true)
            }
            Some(contents) => Ok(&IdentityKey::decode(&contents)? == identity_key),
        }
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
        _ctx: Context,
    ) -> Result<Option<IdentityKey>, SignalProtocolError> {
        let buf = self.get(self.identity_key(address)).map_err(|e| {
            log::error!("error getting identity of {:?}: {}", address, e);
            SignalProtocolError::InvalidState("get_identity", "failed to read identity".into())
        })?;
        Ok(buf.map(|ref b| IdentityKey::decode(b).unwrap()))
    }
}

fn prefix_merge(_key: &[u8], old_value: Option<&[u8]>, merged_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut ret = merged_bytes.to_vec();

    ret.extend_from_slice(old_value.unwrap_or_default());

    Some(ret)
}

fn thread_key(t: &Thread) -> Vec<u8> {
    let mut bytes = SLED_TREE_THREAD_PREFIX.as_bytes().to_owned();
    bytes.append(&mut t.into());
    bytes
}

impl<E: encrypted_sled::Encryption + 'static> MessageStore for EncryptedSledStore<E> {
    type MessagesIter = SledMessagesIter<E>;

    fn save_message(&mut self, thread: &Thread, message: Content) -> Result<(), Error> {
        log::trace!(
            "Storing a message with thread: {:?}, timestamp: {}",
            thread,
            message.metadata.timestamp,
        );

        let tree_thread = self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(thread_key(thread))?;

        let timestamp_bytes = message.metadata.timestamp.to_be_bytes();
        let proto: ContentProto = message.into();
        tree_thread.insert(timestamp_bytes, proto.encode_to_vec())?;
        Ok(())
    }

    fn delete_message(&mut self, thread: &Thread, timestamp: u64) -> Result<bool, Error> {
        Ok(self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(thread_key(thread))?
            .remove(timestamp.to_be_bytes())?
            .is_some())
    }

    fn message(
        &self,
        thread: &Thread,
        timestamp: u64,
    ) -> Result<Option<libsignal_service::prelude::Content>, Error> {
        let tree_thread = self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(thread_key(thread))?;
        // Big-Endian needed, otherwise wrong ordering in sled.
        let val = tree_thread.get(timestamp.to_be_bytes())?;
        if let Some(val) = val {
            let proto = ContentProto::decode(&val[..])?;
            let content = proto.try_into()?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    fn messages(&self, thread: &Thread, from: Option<u64>) -> Result<Self::MessagesIter, Error> {
        let tree_thread = self
            .db
            .read()
            .expect("poisoned mutex")
            .open_tree(thread_key(thread))?;
        let iter = if let Some(from) = from {
            tree_thread.range(..from.to_be_bytes())
        } else {
            tree_thread.range::<&[u8], std::ops::RangeFull>(..)
        }?;
        Ok(SledMessagesIter(iter.rev()))
    }
}

pub struct SledMessagesIter<E>(std::iter::Rev<encrypted_sled::Iter<E>>);

impl<E: encrypted_sled::Encryption + 'static> Iterator for SledMessagesIter<E> {
    // TODO: If error, throw away the rest. Maybe return Result<Content, Error>?
    type Item = Content;

    fn next(&mut self) -> Option<Self::Item> {
        let ivec = self.0.next()?.ok()?.1;
        let proto = ContentProto::decode(&*ivec).ok()?;
        proto.try_into().ok()
    }
}
