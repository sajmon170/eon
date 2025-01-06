use crate::system;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::{Uuid, uuid};
use libp2p::identity::{Keypair, PublicKey, SigningError};

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

#[derive(Serialize, Deserialize, Clone)]
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
}

#[derive(Serialize, Deserialize, Clone)]
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
