//! Minimal JSON-file persistence shared by the local adapters.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::AdapterError;

/// Upper bound for files we read back; our own files are far smaller.
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

/// Loads `path`, or writes and returns `seed()` if the file does not exist yet.
pub fn load_or_seed<T: Serialize + DeserializeOwned>(path: &Path, seed: impl FnOnce() -> T) -> Result<T, AdapterError> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE_BYTES => {
            Err(AdapterError::Storage(format!("{} exceeds the size limit", path.display())))
        }
        Ok(_) => {
            let text = fs::read_to_string(path)?;
            serde_json::from_str(&text).map_err(|e| AdapterError::Storage(format!("{}: {e}", path.display())))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let value = seed();
            save(path, &value)?;
            Ok(value)
        }
        Err(e) => Err(e.into()),
    }
}

/// Writes `value` atomically: to a temporary sibling file first, then renamed
/// over the target, so a crash never leaves a half-written file behind.
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), AdapterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(value).map_err(|e| AdapterError::Storage(e.to_string()))?;
    let temp = temp_path(path);
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(&json)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh, empty directory under the system temp dir for one test.
    pub(crate) fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ourfault-test-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn seeds_missing_files_and_reads_them_back() {
        let path = temp_dir("json").join("nested").join("data.json");
        let first: Vec<u32> = load_or_seed(&path, || vec![1, 2]).unwrap();
        assert_eq!(first, vec![1, 2]);
        save(&path, &vec![3u32]).unwrap();
        let second: Vec<u32> = load_or_seed(&path, || unreachable!()).unwrap();
        assert_eq!(second, vec![3]);
        assert!(!temp_path(&path).exists());
    }

    #[test]
    fn corrupt_files_are_reported_not_overwritten() {
        let path = temp_dir("json-corrupt").join("data.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(load_or_seed::<Vec<u32>>(&path, Vec::new).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
