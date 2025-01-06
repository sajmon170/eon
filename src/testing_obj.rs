use crate::core::*;
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
