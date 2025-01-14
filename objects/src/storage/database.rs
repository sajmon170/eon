use std::collections::HashMap;
use crate::core::object::*;
use uuid::Uuid;
use std::any::Any;

use tracing::info;

#[derive(Default)]
pub struct SystemDatabase {
    objects: HashMap<ObjectId, SignedObject>,
    bindings: HashMap<Uuid, Vec<ObjectId>>,
    tables: HashMap<Uuid, Box<dyn CustomDatabase>>,
    pending_queries: HashMap<ObjectId, Vec<ObjectId>>
}

impl SystemDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_object(&mut self, obj: SignedObject) {
        info!("Added an object.");
        self.objects.insert(obj.get_object_id(), obj);
    }

    pub fn get_object(&self, id: &ObjectId) -> Option<&SignedObject> {
        self.objects.get(id)
    }

    pub fn delete_object(&mut self, id: &ObjectId) {
        self.objects.remove(id);

        for (_, objects) in &mut self.bindings {
            objects.retain(|obj| obj != id);
        }

        for (_, table) in &mut self.tables {
            table.cleanup_id(id);
        }
    }

    pub fn add_binding(&mut self, uuid: Uuid, id: ObjectId) {
        self.bindings
            .entry(uuid)
            .or_default()
            .push(id);
    }

    pub fn get_bindings(&mut self, uuid: Uuid) -> &Vec<ObjectId> {
        self.bindings
            .entry(uuid)
            .or_default()
    }

    pub fn get_table(&mut self, uuid: &Uuid) -> Option<&mut Box<dyn CustomDatabase>> {
        self.tables
            .get_mut(&uuid)
    }

    pub fn insert_table(&mut self, uuid: Uuid, table: Box<dyn CustomDatabase>) {
        self.tables.insert(uuid, table);
    }

    pub fn initialize_query(&mut self, id: &ObjectId, init: Vec<ObjectId>) {
        self.pending_queries.insert(*id, init);
    }
    
    pub fn get_query(&mut self, id: &ObjectId) -> &mut Vec<ObjectId> {
        self.pending_queries
            .entry(*id)
            .or_insert(self.objects.keys().cloned().collect())
    }

    pub fn finish_query(&mut self, id: &ObjectId) -> Option<Vec<ObjectId>> {
        self.pending_queries.remove(id)
    }

    pub fn contains(&mut self, id: &ObjectId) -> bool {
        self.objects.contains_key(id)
    }
}

pub trait CustomDatabase: AsAny + Send + Sync {
    fn cleanup_id(&mut self, _id: &ObjectId) { }
}

pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> AsAny for T
where
    T: 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
