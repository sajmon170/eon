use serde::{Serialize, Deserialize};
use uuid::{Uuid, uuid};
use crate::core::*;

pub type TagId = ObjectId;

#[derive(Serialize, Deserialize)]
pub struct PinObject {
    pub from: ObjectId,
    pub to: SignedObject,
}

impl Typed for PinObject {
    const UUID: Uuid = uuid!("8f729e1f-ea53-4283-b9e2-266ea09302d0");
}

#[derive(Serialize, Deserialize)]
pub struct PinContext {
    pub tag: ObjectId,
    pub pin: PinObject,
}

impl Typed for PinContext {
    const UUID: Uuid = uuid!("73082746-2296-45f8-9114-eb5a6d1bb1fd");
}

#[derive(Serialize, Deserialize)]
pub struct FlatPin {
    pub from: ObjectId,
    pub to: ObjectId,
}

impl Typed for FlatPin {
    const UUID: Uuid = uuid!("37cb84df-1133-44a0-af8d-c61d8fbb2d09");
}

impl PinObject {
    pub fn flatten(&self) -> FlatPin {
        FlatPin {
            from: self.from,
            to: self.to.get_object_id(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tag(String);

impl Typed for Tag {
    const UUID: Uuid = uuid!("65152eb0-896c-4c88-ae27-f077fd5c364d");
}

impl Tag {
    pub fn new(tag: &str) -> Self {
        Self(tag.trim().to_lowercase())
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
