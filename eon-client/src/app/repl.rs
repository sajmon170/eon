use std::{ops::{Deref, DerefMut}, path::PathBuf, str::FromStr, time::Duration};

pub use clap::{Parser, Subcommand};
use objects::prelude::*;

use serde::{Serialize, Deserialize, de};
use serde_with::{base64::Base64, serde_as};

#[derive(Debug, Parser)]
#[command(multicall = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum Command {
    Provide { path: PathBuf },
    Publish { path: PathBuf },
    Get { id: CmdObjectId },
    Wait { time: CmdDuration },
    WaitRandom { time: CmdDuration },
    Quit
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence(Vec<Command>);

impl FromStr for Sequence {
    type Err = serde_yaml::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_yaml::from_str(s)
    }
}

impl<'a> IntoIterator for &'a Sequence {
    type Item = &'a Command; 
    type IntoIter = std::slice::Iter<'a, Command>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for Sequence {
    type Item = Command;
    type IntoIter = std::vec::IntoIter<Command>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdObjectId(#[serde_as(as = "Base64")] ObjectId);

impl FromStr for CmdObjectId {
    type Err = serde_yaml::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_yaml::from_str(s)
    }
}

impl Deref for CmdObjectId {
    type Target = ObjectId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<ObjectId> for CmdObjectId {
    fn from(value: ObjectId) -> Self {
        Self(value)
    }
}

impl From<CmdObjectId> for ObjectId {
    fn from(value: CmdObjectId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub struct CmdDuration(humantime::Duration);

impl Serialize for CmdDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        humantime_serde::serialize(self.0.as_ref(), serializer)
    }
}

impl<'a> de::Deserialize<'a> for CmdDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'a> {
        let duration: Duration = humantime_serde::deserialize(deserializer)?;
        Ok(Self(duration.into()))
    }
}

impl FromStr for CmdDuration {
    type Err = humantime::DurationError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration = humantime::Duration::from_str(s)?;
        Ok(Self(duration))
    }
}

impl Deref for CmdDuration {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
