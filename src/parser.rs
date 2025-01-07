
use uuid::Uuid;
use crate::core::*;
use crate::pins::*;
use crate::testing_obj::*;
use crate::minivault::SystemDatabase;
use crate::system;
use std::collections::HashMap;
use base64::prelude::*;

use tracing::warn;

type ParsingFn = fn(&mut SystemDatabase, &SignedObject) -> Option<ParsingResult>;
type HookFn = fn(&mut SystemDatabase, &SignedObject);

pub struct SystemState {
    parser_map: HashMap<Uuid, ParsingFn>,
    default_parser: ParsingFn,
    db: SystemDatabase,
}

pub struct ParsingResult {
    next: SignedObject,
    hook: Option<HookFn>
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

impl SystemState {
    pub fn new() -> Self {
        Self {
            parser_map: Default::default(),
            default_parser: Self::default_parser,
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

            let parsing_fn = self.parser_map
                .get(next_obj.get_uuid())
                .unwrap_or(&self.default_parser);

            if let Some(result) = parsing_fn(&mut self.db, &next_obj) {
                if let Some(hook) = result.hook {
                    hooks.push(hook);
                }

                next = Some(result.next);
            }
        }
    }
    
    fn default_parser(db: &mut SystemDatabase, obj: &SignedObject) -> Option<ParsingResult> {
        db.add_object(obj.clone());
        None
    }

    pub fn register_parser(&mut self, uuid: Uuid, parser: ParsingFn) {
        self.parser_map.insert(uuid, parser);
    }

    pub fn print_tags(&self, id: &ObjectId) {
        if let Some(tags) = self.db.tag_mapping.get(id) {
            print!("Found tags: ");
            for tag in tags {
                print!("{}", self.db.tags.get(tag).unwrap());
            }

            println!("");
        }
        else {
            println!("No tags found for id [{}]", crate::system::hash_str(id));
        }
    }

    pub fn get_object(&self, id: ObjectId) -> Option<SignedObject> {
        let result = self.db.objects.get(&id).cloned();
        if result.is_none() {
            warn!("Requested object [id: {}] not found",
                  BASE64_STANDARD.encode(&id));
        }
        result
    }
}

pub fn setup_pin_object_parser(state: &mut SystemState) {
    fn setup_pins(db: &mut SystemDatabase, object: &SignedObject) -> Option<ParsingResult> {
        let pin_obj = system::deserialize::<PinObject>(object.get_data());
        let flattened = pin_obj.flatten();
        db.add_pin(flattened);
        
        let included = pin_obj.to;

        // TODO - refactor this
        // maybe implement an AddContext trait?
        // If something can add context then an add_context(objA, objB) function
        // gets called and adds the context to the database
        if *included.get_uuid() == Tag::UUID {
            db.tag_object(pin_obj.from, included.get_object_id());
        }
        
        ParsingResult::new(included).into()
    }

    state.register_parser(PinObject::UUID, setup_pins);
}

pub fn setup_tag_object_parser(state: &mut SystemState) {
    fn setup_tags(db: &mut SystemDatabase, object: &SignedObject) -> Option<ParsingResult> {
        let tag = system::deserialize::<Tag>(object.get_data());
        db.add_tag(object.get_object_id(), tag);
        None
    }
    
    state.register_parser(Tag::UUID, setup_tags);
}

pub fn setup_poem_parser(state: &mut SystemState) {
    fn setup_poems(db: &mut SystemDatabase, object: &SignedObject) -> Option<ParsingResult> {
        db.add_object(object.clone());
        db.add_poem(object.get_object_id());
        None
    }
    
    state.register_parser(Poem::UUID, setup_poems);
}

pub fn setup_file_parser(state: &mut SystemState) {
    fn setup_files(db: &mut SystemDatabase, object: &SignedObject) -> Option<ParsingResult> {
        db.add_object(object.clone());
        db.add_file(object.get_object_id());
        None
    }
    
    state.register_parser(BinaryFile::UUID, setup_files);
}
