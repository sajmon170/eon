use serde::{Serialize, de::DeserializeOwned};
use base64::prelude::*;

pub type Hash = blake3::Hash;

pub fn hash(data: Vec<u8>) -> Hash {
    blake3::hash(&data)
}

pub fn serialize<T: Serialize + DeserializeOwned>(obj: &T) -> Vec<u8> {
    postcard::to_allocvec(obj).unwrap()
}

pub fn deserialize<T: Serialize + DeserializeOwned>(bytes: &Vec<u8>) -> T {
    postcard::from_bytes(bytes).unwrap()
}

pub fn hash_str(hash: &[u8; 32]) -> String {
    BASE64_STANDARD.encode(hash)
}
