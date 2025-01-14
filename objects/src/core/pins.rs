use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use crate::core::object::*;
use crate::core::query::*;
use crate::storage::database::{SystemDatabase, CustomDatabase};
use crate::storage::object_parser::ObjectParser;

#[derive(Serialize, Deserialize)]
pub struct PinObject {
    pub from: ObjectId,
    pub to: SignedObject,
}

impl Typed for PinObject {
    const UUID: Uuid = uuid!("8f729e1f-ea53-4283-b9e2-266ea09302d0");
}

impl Parsable for PinObject {
    fn parse(db: &mut SystemDatabase, _id: ObjectId, pin: Self) -> Option<ParsingResult> {
        let flattened = pin.flatten();

        Self::extract_custom_db(db).unwrap()
            .links.push(flattened);
        
        let included = pin.to;
        
        ParsingResult::new(included).into()
    }

    fn maybe_custom_db() -> Option<Box<dyn CustomDatabase>> {
        Some(Box::new(Self::init_custom_db()))
    }
}

#[derive(Default)]
pub struct PinDb {
    links: Vec<FlatPin>
}

impl CustomDatabase for PinDb {
    fn cleanup_id(&mut self, id: &ObjectId) {
        self.links.retain(|flatpin| flatpin.to != *id && flatpin.from != *id);
    }
}

impl UseCustomDb for PinObject {
    type Database = PinDb;

    fn init_custom_db() -> Self::Database {
        Self::Database::default()
    }
}

impl PinDb {
    fn get_pinned(&self, id: &ObjectId) -> Vec<ObjectId> {
        self.links.iter()
            .filter_map(|link| if &link.from == id { Some(link.to) } else { None })
            .collect()
    }
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
pub struct PinnedWith(ObjectId);

impl Typed for PinnedWith {
    const UUID: Uuid = uuid!("14810393-ecc4-437e-bfc5-ad2657d9eb60");
}

impl Query for PinnedWith {
    fn parse(parser: &mut ObjectParser, id: ObjectId, query: Self) -> Vec<ObjectId> {
        let pin_db = PinObject::extract_custom_db(&mut parser.db).unwrap();
        let pinned = pin_db.get_pinned(&query.0);

        parser.db.get_query(&id)
            .retain(|id| pinned.contains(&id));

        parser.db.finish_query(&id).unwrap_or_default()
    }
}

impl PinnedWith {
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tag(String);

impl Typed for Tag {
    const UUID: Uuid = uuid!("65152eb0-896c-4c88-ae27-f077fd5c364d");
}

impl Parsable for Tag { }

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
