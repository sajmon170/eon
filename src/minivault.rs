use std::collections::HashMap;
use crate::core::*;
use crate::pins::*;
use crate::testing_obj::*;
use crate::system::hash_str;

#[derive(Default)]
pub struct SystemDatabase {
    pub objects: HashMap<ObjectId, SignedObject>,
    pub pins: Vec<FlatPin>,
    pub tags: HashMap<TagId, Tag>,
    pub tag_mapping: HashMap<ObjectId, Vec<TagId>>,
    pub poems: HashMap<ObjectId, Poem>
}

impl SystemDatabase {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_tag(&mut self, obj_id: ObjectId, tag: Tag) {
        println!("Added a tag: \"{tag}\" ([{}])", hash_str(&obj_id));
        self.tags.insert(obj_id, tag);
    }

    pub fn tag_object(&mut self, obj_id: ObjectId, tag_id: TagId) {
        println!("Tagged [{}] with [{}]", hash_str(&obj_id), hash_str(&tag_id));
        self.tag_mapping
            .entry(obj_id)
            .or_insert(Vec::new())
            .push(tag_id);
    }

    pub fn add_pin(&mut self, pin: FlatPin) {
        println!("Added a pin.");
        self.pins
            .push(pin);
    }

    pub fn add_poem(&mut self, obj_id: ObjectId, poem: Poem) {
        println!("Added a poem [{}]:", hash_str(&obj_id));
        println!("{poem}");
        self.poems.insert(obj_id, poem);
    }

    pub fn add_object(&mut self, obj: SignedObject) {
        println!("Added an object.");
        self.objects.insert(obj.get_object_id(), obj);
    }
}
