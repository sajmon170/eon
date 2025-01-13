use uuid::Uuid;
use crate::core::*;
use crate::pins::*;
use crate::testing_obj::*;
use crate::minivault::SystemDatabase;
use crate::system;
use crate::parsing::*;
use std::collections::{HashMap, VecDeque};
use std::any::Any;
use base64::prelude::*;

use tracing::warn;

pub struct ObjectParser {
    object_parser_map: HashMap<Uuid, ParsingFn>,
    rpc_parser_map: HashMap<Uuid, RpcFn>,
    query_parser_map: HashMap<Uuid, QueryFn>,
    default_object_parser: ParsingFn,
    default_rpc_parser: RpcFn,
    default_query_parser: QueryFn,
    pub db: SystemDatabase,
}

impl ObjectParser {
    pub fn new() -> Self {
        Self {
            object_parser_map: Default::default(),
            rpc_parser_map: Default::default(),
            query_parser_map: Default::default(),
            default_object_parser: Self::default_object_parser,
            default_rpc_parser: Self::default_rpc_parser,
            default_query_parser: Self::default_query_parser,
            db: Default::default()
        }
    }
    
    pub fn parse_object(&mut self, obj: SignedObject) {
        let mut hooks = Vec::<HookFn>::new();
        let mut next = Some(obj);

        while let Some(next_obj) = next.take() {
            for hook in &mut *hooks {
                hook(&mut self.db, &next_obj)
            }

            let parsing_fn = self.get_object_parser(next_obj.get_uuid());

            if let Some(result) = parsing_fn(&mut self.db, &next_obj) {
                if let Some(hook) = result.hook {
                    hooks.push(hook);
                }

                next = Some(result.next);
            }
        }
    }

    pub fn parse_rpc(&mut self, rpc: TypedObject) -> Option<Vec<SignedObject>> {
        let parsing_fn = self.get_rpc_parser(&rpc.uuid);
        let mut parsing_result = None;

        if let Some(result) = parsing_fn(&mut self.db, &rpc) {
            match result {
                RpcTask::ParseObject(obj) => self.parse_object(obj),
                RpcTask::Delete(id) => self.db.delete_object(&id),
                RpcTask::Return(ids) => {
                    parsing_result = Some(self.into_objects(ids));
                }
                RpcTask::ContinueAsQuery => {
                    let ids = self.parse_query(rpc);
                    parsing_result = Some(self.into_objects(ids));
                }
            }
        }

        parsing_result
    }
    
    fn into_objects(&self, ids: Vec<ObjectId>) -> Vec<SignedObject> {
        ids.iter()
            .flat_map(|id| self.db.get_object(id).cloned())
            .collect()
    }

    pub fn parse_query(&mut self, query: TypedObject) -> Vec<ObjectId> {
        let parsing_fn = self.get_query_parser(&query.uuid);
        parsing_fn(self, &query)
    }

    pub fn get_object_parser(&self, uuid: &Uuid) -> &ParsingFn {
        self.object_parser_map
            .get(uuid)
            .unwrap_or(&self.default_object_parser)
    }
 
    pub fn get_rpc_parser(&self, uuid: &Uuid) -> &RpcFn {
        self.rpc_parser_map
            .get(uuid)
            .unwrap_or(&self.default_rpc_parser)
    }

    pub fn get_query_parser(&self, uuid: &Uuid) -> &QueryFn {
        self.query_parser_map
            .get(uuid)
            .unwrap_or(&self.default_query_parser)
    }
    
    fn default_object_parser(db: &mut SystemDatabase, obj: &SignedObject) -> Option<ParsingResult> {
        db.add_object(obj.clone());
        None
    }

    fn default_rpc_parser(db: &mut SystemDatabase, rpc: &TypedObject) -> Option<RpcTask> {
        Some(RpcTask::ContinueAsQuery)
    }

    fn default_query_parser(parser: &mut Self, query: &TypedObject) -> Vec<ObjectId> {
        Vec::new()
    }

    pub fn register_object_parser<T: Parsable + 'static>(&mut self) {
        self.object_parser_map.insert(T::UUID, T::store_and_parse);
        
        if let Some(table) = T::maybe_custom_db() {
            self.db.insert_table(T::UUID, table);
        }
    }

    pub fn register_rpc_parser<T: Rpc + 'static>(&mut self) {
        self.rpc_parser_map.insert(T::UUID, T::deserialize_and_parse);
    }

    pub fn register_query_parser<T: Query + 'static>(&mut self) {
        self.query_parser_map.insert(T::UUID, T::deserialize_and_parse);
    }

    pub fn get_object(&self, id: ObjectId) -> Option<SignedObject> {
        let result = self.db.get_object(&id).cloned();
        if result.is_none() {
            warn!("Requested object [id: {}] not found",
                  BASE64_STANDARD.encode(&id));
        }
        result
    }
}
