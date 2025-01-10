use crate::core::*;
use crate::parsing::*;
use serde::{Serialize, Deserialize};
use heck::AsTitleCase;
use uuid::{Uuid, uuid};
use std::fmt::{Display, Error, Formatter};

#[derive(Serialize, Deserialize)]
pub struct Author {
    name: String,
    surname: String
}

impl Author {
    pub fn new(name: &str, surname: &str) -> Self {
        Self {
            name: name.trim().to_lowercase(),
            surname: surname.trim().to_lowercase()
        }
    }
}

impl Display for Author {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{} {}", AsTitleCase(&self.name), AsTitleCase(&self.surname))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Poem {
    author: Author,
    content: String
}

impl Poem {
    pub fn new(author: Author, content: String) -> Self {
        Self {
            author,
            content
        }
    } 
}

impl Display for Poem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}:\n\"{}\"", self.author, self.content)
    }
}

impl Typed for Poem {
    const UUID: Uuid = uuid!("d630cef4-b8e2-4c96-823b-28df2445818a");
}

impl Parsable for Poem { }

#[derive(Serialize, Deserialize)]
pub struct BinaryFile {
    pub filename: String,
    pub bytes: Vec<u8>
}

impl Typed for BinaryFile {
    const UUID: Uuid = uuid!("d5003cdd-1076-4554-b195-b6907207afca");
}

impl Parsable for BinaryFile { }

impl BinaryFile {
    // TODO - handle errors
    pub fn new(path: &std::path::Path) -> Self {
        let filename = path.file_name().unwrap().to_owned().into_string().unwrap();
        let bytes = std::fs::read(path).unwrap();

        Self {
            filename,
            bytes
        }
    }

    pub fn save(&self, path: &std::path::Path) {
        std::fs::write(path.join(&self.filename), &self.bytes).unwrap();
    }
}
