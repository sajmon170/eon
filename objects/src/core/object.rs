use crate::system;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;
use libp2p_identity::{Keypair, PublicKey, SigningError};
use crate::storage::database::{SystemDatabase, CustomDatabase};

pub type ObjectId = [u8; 32];
pub type Signature = Vec<u8>;

pub trait Typed: Serialize + DeserializeOwned {
    const UUID: Uuid;

    fn make_typed(&self) -> TypedObject {
        TypedObject {
            uuid: Self::UUID,
            data: system::serialize(self)
        }
    }
}

impl<T: Typed> From<T> for TypedObject {
    fn from(value: T) -> Self {
        value.make_typed()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedObject {
    pub uuid: Uuid,
    pub data: Vec<u8>,
}

impl TypedObject {
    pub fn sign(&self, keypair: &Keypair) -> Result<SignedObject, SigningError> {
        let signature = keypair.sign(&system::serialize(self))?;
        Ok(SignedObject {
            object: self.clone(),
            signature
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        system::serialize(self)
    }

    pub fn get_object_id(&self) -> ObjectId {
        system::hash(self.serialize()).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}

impl SignedObject {
    pub fn get_uuid(&self) -> &Uuid {
        &self.object.uuid
    }

    pub fn serialize(&self) -> Vec<u8> {
        system::serialize(self)
    }

    pub fn get_object_id(&self) -> ObjectId {
        system::hash(self.serialize()).into()
    }

    pub fn get_data(&self) -> &Vec<u8> {
        &self.object.data
    }

    pub fn verify(&self, public: PublicKey) -> bool {
        public.verify(&system::serialize(&self.object), &self.signature)
    }
}

pub type ParsingFn = fn(&mut SystemDatabase, &SignedObject) -> Option<ParsingResult>;
pub type HookFn = fn(&mut SystemDatabase, &SignedObject);

pub struct ParsingResult {
    pub next: SignedObject,
    pub hook: Option<HookFn>
}

impl ParsingResult {
    pub fn new(object: SignedObject) -> Self {
        ParsingResult {
            next: object,
            hook: None
        }
    }

    pub fn with_hook(mut self, func: HookFn) -> Self {
        self.hook = Some(func);
        self
    }
}

pub trait Parsable: Typed {
    fn parse(_db: &mut SystemDatabase, _id: ObjectId, _obj: Self) -> Option<ParsingResult> {
        None
    }

    fn store_and_parse(db: &mut SystemDatabase, obj: &SignedObject) -> Option<ParsingResult> {
        let deserialized = system::deserialize::<Self>(obj.get_data());
        db.add_object(obj.clone());

        let id = obj.get_object_id();
        db.add_binding(Self::UUID, id);

        Self::parse(db, id, deserialized)
    }

    fn maybe_custom_db() -> Option<Box<dyn CustomDatabase>> {
        None
    }
}

pub trait UseCustomDb: Typed {
    type Database: CustomDatabase + 'static;

    fn init_custom_db() -> Self::Database;

    fn extract_custom_db(db: &mut SystemDatabase) -> Option<&mut Self::Database> {
        db.get_table(&Self::UUID)
            .and_then(|entry| entry.as_any_mut().downcast_mut::<Self::Database>())
    }
}
