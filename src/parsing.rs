use crate::core::*;
pub use crate::minivault::{SystemDatabase, CustomDatabase};
use crate::system;
use serde::{Serialize, Deserialize};
use uuid::{Uuid, uuid};
use std::slice::Iter;

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
    fn parse(db: &mut SystemDatabase, id: ObjectId, obj: Self) -> Option<ParsingResult> {
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

pub trait UseCustomDb: Typed + Parsable {
    type Database: CustomDatabase + 'static;

    fn init_custom_db() -> Self::Database;

    fn extract_custom_db(db: &mut SystemDatabase) -> Option<&mut Self::Database> {
        db.get_table(&Self::UUID)
            .and_then(|entry| entry.as_any_mut().downcast_mut::<Self::Database>())
    }
}

pub type QueryFn = fn(&mut SystemDatabase, &TypedObject) -> Option<QueryResult>;

pub trait Query: Typed {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<QueryResult>;

    fn deserialize_and_parse(db: &mut SystemDatabase, query: &TypedObject)
                             -> Option<QueryResult> {
        let deserialized = system::deserialize::<Self>(&query.data);
        Self::parse(db, query.get_object_id(), deserialized)
    }
}

pub enum QueryResult {
    Return(Vec<ObjectId>),
    Parse(SignedObject),
    Delete(ObjectId),
}

#[derive(Serialize, Deserialize)]
pub struct GetObject(ObjectId);

impl Typed for GetObject {
    const UUID: Uuid = uuid!("4506bd52-3ee7-46b9-b55b-03d532a2018d");
}

impl Query for GetObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<QueryResult> {
        if db.contains(&query.0) {
            let mut to_return = Vec::new();
            to_return.push(query.0);
            Some(QueryResult::Return(to_return))
        }
        else {
            None
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct StoreObject(SignedObject);

impl Typed for StoreObject {
    const UUID: Uuid = uuid!("73049794-ad5d-48b6-9854-f2c02e6bf50d");
}

impl Query for StoreObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<QueryResult> {
        Some(QueryResult::Parse(query.0))
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeleteObject(ObjectId);

impl Typed for DeleteObject {
    const UUID: Uuid = uuid!("9136dfcf-d9f3-4051-ac8d-3879828db933");
}

impl Query for DeleteObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<QueryResult> {
        Some(QueryResult::Delete(query.0))
    }
}
