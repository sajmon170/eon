use crate::system;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use crate::core::object::*;
use crate::storage::database::{SystemDatabase, CustomDatabase};

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
    fn parse(db: &mut SystemDatabase, _id: ObjectId, query: Self) -> Option<RpcTask> {
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
    fn parse(_db: &mut SystemDatabase, _id: ObjectId, query: Self) -> Option<RpcTask> {
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
    fn parse(_db: &mut SystemDatabase, _id: ObjectId, query: Self) -> Option<RpcTask> {
        Some(RpcTask::Delete(query.0))
    }
}

impl DeleteObject {
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }
}
