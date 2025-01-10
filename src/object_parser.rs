use uuid::Uuid;
use crate::core::*;
use crate::pins::*;
use crate::testing_obj::*;
use crate::minivault::SystemDatabase;
use crate::system;
use crate::parsing::*;
use std::collections::HashMap;
use std::any::Any;
use base64::prelude::*;

use tracing::warn;

pub struct ObjectParser {
    object_parser_map: HashMap<Uuid, ParsingFn>,
    query_parser_map: HashMap<Uuid, QueryFn>,
    default_object_parser: ParsingFn,
    default_query_parser: QueryFn,
    db: SystemDatabase,
}

impl ObjectParser {
    pub fn new() -> Self {
        Self {
            object_parser_map: Default::default(),
            query_parser_map: Default::default(),
            default_object_parser: Self::default_object_parser,
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

    pub fn parse_query(&mut self, query: TypedObject) -> Option<Vec<SignedObject>> {
        // Query parsing
        // Move the previous query result into the next query field
        
        
        //let result = self.query_parser_map.get(k)
        let parsing_fn = self.get_query_parser(&query.uuid);

        let mut parsing_result = None;
        
        if let Some(query_result) = parsing_fn(&mut self.db, &query) {
            match query_result {
                QueryResult::Parse(obj) => self.parse_object(obj),
                QueryResult::Delete(id) => self.db.delete_object(&id),
                QueryResult::Return(ids) => {
                    let objects: Vec<SignedObject> = ids
                        .iter()
                        .flat_map(|id| self.db.get_object(id).cloned())
                        .collect();

                    parsing_result = Some(objects);
                }
            }
        }

        parsing_result
    }

    fn get_object_parser(&self, uuid: &Uuid) -> &ParsingFn {
        self.object_parser_map
            .get(uuid)
            .unwrap_or(&self.default_object_parser)
    }

    fn get_query_parser(&self, uuid: &Uuid) -> &QueryFn {
        self.query_parser_map
            .get(uuid)
            .unwrap_or(&self.default_query_parser)
    }
    
    fn default_object_parser(db: &mut SystemDatabase, obj: &SignedObject) -> Option<ParsingResult> {
        db.add_object(obj.clone());
        None
    }

    fn default_query_parser(db: &mut SystemDatabase, query: &TypedObject) -> Option<QueryResult> {
        None
    }

    pub fn register_object_parser<T: Parsable + 'static>(&mut self) {
        self.object_parser_map.insert(T::UUID, T::store_and_parse);
        
        if let Some(table) = T::maybe_custom_db() {
            self.db.insert_table(T::UUID, table);
        }
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
