use std::collections::HashMap;
use crate::core::*;
use crate::pins::*;
use crate::testing_obj::*;
use crate::system::{hash_str, self};

use tracing::info;

#[derive(Default)]
pub struct SystemDatabase {
    pub objects: HashMap<ObjectId, SignedObject>,
    pub pins: Vec<FlatPin>,
    pub tags: HashMap<TagId, Tag>,
    pub tag_mapping: HashMap<ObjectId, Vec<TagId>>,
    // These ObjectIds MUST be stored inside the objects hashmap.
    pub poems: Vec<ObjectId>,
    pub files: Vec<ObjectId>
}

impl SystemDatabase {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_tag(&mut self, obj_id: ObjectId, tag: Tag) {
        info!("Added a tag: \"{tag}\" ([{}])", hash_str(&obj_id));
        self.tags.insert(obj_id, tag);
    }

    pub fn tag_object(&mut self, obj_id: ObjectId, tag_id: TagId) {
        info!("Tagged [{}] with [{}]", hash_str(&obj_id), hash_str(&tag_id));
        self.tag_mapping
            .entry(obj_id)
            .or_insert(Vec::new())
            .push(tag_id);
    }

    pub fn add_pin(&mut self, pin: FlatPin) {
        info!("Added a pin.");
        self.pins
            .push(pin);
    }

    pub fn add_poem(&mut self, obj_id: ObjectId) {
        info!("Added a poem [{}]:", hash_str(&obj_id));
        self.poems.push(obj_id);
    }

    pub fn add_file(&mut self, obj_id: ObjectId) {
        info!("Added a file [{}]:", hash_str(&obj_id));
        self.files.push(obj_id);
    }

    pub fn add_object(&mut self, obj: SignedObject) {
        info!("Added an object.");
        self.objects.insert(obj.get_object_id(), obj);
    }

    pub fn get_file(&mut self, obj_id: ObjectId) -> Option<BinaryFile> {
        if self.files.contains(&obj_id) {
            let obj = self.objects.get(&obj_id).unwrap();
            Some(system::deserialize::<BinaryFile>(obj.get_data()))
        }
        else {
            None
        }
    }
}
