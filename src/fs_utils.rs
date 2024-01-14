use crate::{
    compressor::Compressor,
    constants::{GIT_BASE_DIR, GIT_OBJECTS_DIR},
    git_object::{
        tree_line::{TREE_LINE_MODE_FILE, TREE_LINE_MODE_FOLDER},
        GitObject, GIT_OBJECT_TYPE_BLOB, GIT_OBJECT_TYPE_TREE,
    },
    hasher::create_hash,
};
use std::{fs, vec};

pub struct FsUtils {}

impl FsUtils {
    pub fn read_bytes_for_hash(hash: &str) -> anyhow::Result<Vec<u8>> {
        // Checks
        if hash.len() < 6 {
            return Err(anyhow::anyhow!("Invalid hash"));
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
            return Err(anyhow::anyhow!("Invalid hash"));
        };
        // Create path
        let file_path: std::path::PathBuf = file_fs_dir_entry.path();
        let Some(file_path) = file_path.to_str() else {
            return Err(anyhow::anyhow!("Invalid hash"));
        };
        // Read bytes
        let file_bytes = fs::read(file_path)?;
        Ok(file_bytes)
    }

    pub fn write_to_fs(git_object: GitObject) -> anyhow::Result<String> {
        // Create object content
        let (hash, compressed_data) = git_object.to_raw()?;
        // Write
        let dir_path = format!("{GIT_OBJECTS_DIR}/{}", &hash[..2]);
        if !fs::metadata(dir_path.clone()).is_ok() {
            fs::create_dir(&dir_path)?;
        }
        let object_path = format!("{dir_path}/{}", &hash[2..]);
        fs::write(&object_path, compressed_data)?;
        // Return hash
        Ok(hash)
    }

    pub fn write_tree(path: String) -> anyhow::Result<Vec<u8>> {
        let mut tree_bytes: Vec<u8> = vec![];

        let entries = fs::read_dir(path)?;
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            // Get path
            let entry_path = entry.path();

            // If it's the git directory, skip it
            if entry_path.ends_with(GIT_BASE_DIR) {
                continue;
            }
            let file_name = entry.file_name();
            let Ok(file_name_string) = file_name.into_string() else {
                continue;
            };

            // Directory
            if entry_path.is_dir() {
                let Ok(entry_path_string) = entry_path.clone().into_os_string().into_string()
                else {
                    continue;
                };
                let header = format!("{} {}\0", TREE_LINE_MODE_FOLDER, file_name_string);
                tree_bytes.extend(header.bytes());
                let hash = Self::write_tree(entry_path_string)?;
                tree_bytes.extend(&hash);
                continue;
            }

            // File
            let header = format!("{} {}\0", TREE_LINE_MODE_FILE, file_name_string);
            tree_bytes.extend(header.bytes());
            let file_bytes = fs::read(entry_path)?;
            let hash = Self::write_object(GIT_OBJECT_TYPE_BLOB, &file_bytes)?;
            tree_bytes.extend(&hash);
        }

        // Write tree
        let hash = Self::write_object(GIT_OBJECT_TYPE_TREE, &tree_bytes)?;

        Ok(hash)
    }

    fn write_object(object_type: &str, content_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
        let header = format!("{object_type} {}\0", content_bytes.len());
        let object_bytes = [header.as_bytes(), &content_bytes].concat();

        // Hash
        let hash = create_hash(&object_bytes);
        let hash_hex = hex::encode(&hash);

        // Compress
        let compressed_object_bytes = Compressor::compress(&object_bytes)?;

        // Write
        let dir_path = format!("{GIT_OBJECTS_DIR}/{}", &hash_hex[..2]);
        if !fs::metadata(&dir_path).is_ok() {
            fs::create_dir(&dir_path)?;
        }
        let object_path = format!("{dir_path}/{}", &hash_hex[2..]);
        fs::write(&object_path, compressed_object_bytes)?;

        Ok(hash)
    }
}
