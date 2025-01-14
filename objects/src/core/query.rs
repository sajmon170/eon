use crate::core::object::*;
use crate::storage::database::CustomDatabase;
use crate::system;
use crate::storage::object_parser::ObjectParser;
use serde::{Serialize, Deserialize};
use uuid::{Uuid, uuid};

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
