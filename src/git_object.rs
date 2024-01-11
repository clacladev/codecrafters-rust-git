use crate::constants::GIT_OBJECTS_DIR;
use flate2::{read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
};

pub const GIT_OBJECT_TYPE_BLOB: &str = "blob";
pub const GIT_OBJECT_TYPE_TREE: &str = "tree";

pub enum GitObject {
    Blob(String),
    Tree(String),
}

impl GitObject {
    pub fn object_type(&self) -> String {
        match self {
            GitObject::Blob(_) => GIT_OBJECT_TYPE_BLOB.to_string(),
            GitObject::Tree(_) => GIT_OBJECT_TYPE_TREE.to_string(),
        }
    }
}

impl GitObject {
    pub fn new(object_type: &str, content_string: &str) -> anyhow::Result<Self> {
        let content_string = content_string.to_string();
        match object_type {
            GIT_OBJECT_TYPE_BLOB => Ok(GitObject::Blob(content_string)),
            GIT_OBJECT_TYPE_TREE => Ok(GitObject::Tree(content_string)),
            _ => Err(anyhow::anyhow!(format!(
                "Invalid object type {}",
                object_type
            ))),
        }
    }

    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        // Decompress
        let mut decoder = ZlibDecoder::new(bytes);
        let mut decompressed_bytes = vec![];
        decoder.read_to_end(&mut decompressed_bytes)?;
        let content_string = String::from_utf8_lossy(decompressed_bytes.as_slice()).to_string();

        // Get type
        let Some((object_type, content_string)) = content_string.split_once(' ') else {
            return Err(anyhow::anyhow!("Failed to read object type"));
        };
        // Get content
        let Some((_, content_string)) = content_string.split_once('\0') else {
            return Err(anyhow::anyhow!("Failed to read object length"));
        };
        // Create object
        GitObject::new(object_type, content_string)
    }

    fn from_path(path: &str) -> anyhow::Result<Self> {
        let file_bytes = fs::read(path)?;
        GitObject::from_bytes(file_bytes.as_slice())
    }

    pub fn from_hash(hash: &str) -> anyhow::Result<Self> {
        // Checks
        if hash.len() < 6 {
            return Err(anyhow::anyhow!("Invalid tree hash"));
        }
        // Find file starting with hash
        let mut dir_iterator = fs::read_dir(format!("{GIT_OBJECTS_DIR}/{}/", &hash[..2]))?;
        let Some(Ok(file_fs_dir_entry)) = dir_iterator.find(|entry| {
            let Ok(entry) = entry.as_ref() else {
                return false;
            };
            let Ok(entry_name) = entry.file_name().into_string() else {
                return false;
            };
            entry_name.starts_with(&hash[2..])
        }) else {
            return Err(anyhow::anyhow!("Invalid tree hash"));
        };
        // Create path
        let file_path: std::path::PathBuf = file_fs_dir_entry.path();
        let Some(file_path) = file_path.to_str() else {
            return Err(anyhow::anyhow!("Invalid tree hash"));
        };
        // Create object
        GitObject::from_path(file_path)
    }
}

impl GitObject {
    fn to_raw(&self) -> anyhow::Result<(String, Vec<u8>)> {
        let object_type = self.object_type();
        let content_string = match self {
            GitObject::Blob(content_string) => content_string,
            GitObject::Tree(content_string) => content_string,
        };
        let header = format!("{object_type} {}\0", content_string.len());
        let content = [header.as_bytes(), content_string.as_bytes()].concat();

        // Hash
        let mut hasher = Sha1::new();
        hasher.update(content.as_slice());
        let hash = hex::encode(hasher.finalize());

        // Compress
        let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&content)?;
        let compressed_data = encoder.finish()?;
        Ok((hash, compressed_data))
    }

    pub fn write_to_fs(&self) -> anyhow::Result<String> {
        // Create object content
        let (hash, compressed_data) = self.to_raw()?;
        // Write
        let dir_path = format!("{GIT_OBJECTS_DIR}/{}", &hash[..2]);
        fs::create_dir(&dir_path)?;
        let object_path = format!("{dir_path}/{}", &hash[2..]);
        fs::write(&object_path, compressed_data)?;
        Ok(hash)
    }
}
