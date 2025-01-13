use crate::core::*;
pub use crate::minivault::{SystemDatabase, CustomDatabase};
use crate::system;
pub use crate::object_parser::ObjectParser;
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

pub trait UseCustomDb: Typed {
    type Database: CustomDatabase + 'static;

    fn init_custom_db() -> Self::Database;

    fn extract_custom_db(db: &mut SystemDatabase) -> Option<&mut Self::Database> {
        db.get_table(&Self::UUID)
            .and_then(|entry| entry.as_any_mut().downcast_mut::<Self::Database>())
    }
}

pub type RpcFn = fn(&mut SystemDatabase, &TypedObject) -> Option<RpcTask>;

pub trait Rpc: Typed {
    fn parse(db: &mut SystemDatabase, rpc_id: ObjectId, rpc: Self) -> Option<RpcTask>;

    fn deserialize_and_parse(db: &mut SystemDatabase, rpc: &TypedObject)
                             -> Option<RpcTask> {
        let deserialized = system::deserialize::<Self>(&rpc.data);
        Self::parse(db, rpc.get_object_id(), deserialized)
    }

    fn maybe_custom_db() -> Option<Box<dyn CustomDatabase>> {
        None
    }
}

pub enum RpcTask {
    Return(Vec<ObjectId>),
    ParseObject(SignedObject),
    Delete(ObjectId),
    ContinueAsQuery
}

#[derive(Serialize, Deserialize)]
pub struct GetObject(ObjectId);

impl Typed for GetObject {
    const UUID: Uuid = uuid!("4506bd52-3ee7-46b9-b55b-03d532a2018d");
}

impl Rpc for GetObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<RpcTask> {
        if db.contains(&query.0) {
            let mut to_return = Vec::new();
            to_return.push(query.0);
            Some(RpcTask::Return(to_return))
        }
        else {
            None
        }
    }
}

impl GetObject {
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize)]
pub struct StoreObject(SignedObject);

impl Typed for StoreObject {
    const UUID: Uuid = uuid!("73049794-ad5d-48b6-9854-f2c02e6bf50d");
}

impl Rpc for StoreObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<RpcTask> {
        Some(RpcTask::ParseObject(query.0))
    }
}

impl StoreObject {
    pub fn new(obj: SignedObject) -> Self {
        Self(obj)
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeleteObject(ObjectId);

impl Typed for DeleteObject {
    const UUID: Uuid = uuid!("9136dfcf-d9f3-4051-ac8d-3879828db933");
}

impl Rpc for DeleteObject {
    fn parse(db: &mut SystemDatabase, id: ObjectId, query: Self) -> Option<RpcTask> {
        Some(RpcTask::Delete(query.0))
    }
}

impl DeleteObject {
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }
}

//pub type QueryFn = fn(&mut SystemDatabase, &TypedObject, &ObjectParser) -> Vec<ObjectId>;
pub type QueryFn = fn(&mut ObjectParser, &TypedObject) -> Vec<ObjectId>;

pub trait Query: Typed {
    fn parse(parser: &mut ObjectParser, query_id: ObjectId, query: Self) -> Vec<ObjectId>;

    fn deserialize_and_parse(parser: &mut ObjectParser, query: &TypedObject)
                             -> Vec<ObjectId> {
        let deserialized = system::deserialize::<Self>(&query.data);
        Self::parse(parser, query.get_object_id(), deserialized)
    }

    fn maybe_custom_db() -> Option<Box<dyn CustomDatabase>> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct CompositeQuery {
    queries: Vec<TypedObject>
}

impl Typed for CompositeQuery {
    const UUID: Uuid = uuid!("2fdd5632-eec8-4c87-990d-1ac72cae0bbc");
}

impl Query for CompositeQuery {
    fn parse(parser: &mut ObjectParser, id: ObjectId, query: Self) -> Vec<ObjectId> {
        for query in query.queries {
            let current = parser.db.get_query(&id).clone();
            parser.db.initialize_query(&query.get_object_id(), current);
            
            let parsing_result = parser.parse_query(query);
            parser.db.initialize_query(&id, parsing_result);
        }

        parser.db.finish_query(&id).unwrap_or_default()
    }
}

impl CompositeQuery {
    pub fn new(queries: Vec<TypedObject>) -> Self {
        Self { queries }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WithUuid(Uuid);

impl Typed for WithUuid {
    const UUID: Uuid = uuid!("d0805046-218f-425a-b4ae-63e5535ff6da");
}

impl Query for WithUuid {
    fn parse(parser: &mut ObjectParser, id: ObjectId, query: Self) -> Vec<ObjectId> {
        let mut ids = parser.db.get_query(&id).clone();
        ids.retain(|id| parser.db.get_object(&id).unwrap().get_uuid() == &query.0);

        parser.db.finish_query(&id);
        ids
    }
}

impl WithUuid {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
