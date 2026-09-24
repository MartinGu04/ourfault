//! Minimal JSON-file persistence shared by the local adapters.
//!
//! Every file carries a `schemaVersion`. Files written by an older or newer
//! version of OurFault are reported as errors and never overwritten.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::AdapterError;

/// Version of the local data layout. Version 1 (PR #1) used other file
/// names and is ignored; see docs/ARCHITECTURE.md, "Local data and migration".
pub const SCHEMA_VERSION: u32 = 2;

/// Upper bound for files we read back; our own files are far smaller.
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Versioned<T> {
    schema_version: u32,
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionOnly {
    schema_version: Option<u32>,
}

/// Reads a versioned file. `Ok(None)` if it does not exist.
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, AdapterError> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE_BYTES => {
            Err(AdapterError::Storage(format!("{} exceeds the size limit", path.display())))
        }
        Ok(_) => {
            let text = fs::read_to_string(path)?;
            let corrupt = |e: serde_json::Error| AdapterError::Storage(format!("{}: {e}", path.display()));
            let version: VersionOnly = serde_json::from_str(&text).map_err(corrupt)?;
            if version.schema_version != Some(SCHEMA_VERSION) {
                return Err(AdapterError::Storage(format!(
                    "{}: unsupported schema version {:?}",
                    path.display(),
                    version.schema_version
                )));
            }
            let versioned: Versioned<T> = serde_json::from_str(&text).map_err(corrupt)?;
            Ok(Some(versioned.data))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Loads `path`, or writes and returns `seed()` if the file does not exist yet.
pub fn load_or_seed<T: Serialize + DeserializeOwned>(path: &Path, seed: impl FnOnce() -> T) -> Result<T, AdapterError> {
    match load(path)? {
        Some(value) => Ok(value),
        None => {
            let value = seed();
            save(path, &value)?;
            Ok(value)
        }
    }
}

/// Writes `value` atomically: to a temporary sibling file first, then renamed
/// over the target, so a crash never leaves a half-written file behind.
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), AdapterError> {
    let json = serde_json::to_vec_pretty(&Versioned { schema_version: SCHEMA_VERSION, data: value })
        .map_err(|e| AdapterError::Storage(e.to_string()))?;
    write_atomic(path, &json)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AdapterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = temp_path(path);
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

/// Serialises `value` in the versioned format (for files created with
/// create-if-absent semantics).
pub fn to_versioned_json<T: Serialize>(value: &T) -> Result<Vec<u8>, AdapterError> {
    serde_json::to_vec_pretty(&Versioned { schema_version: SCHEMA_VERSION, data: value })
        .map_err(|e| AdapterError::Storage(e.to_string()))
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// A file name that is safe on Windows and in SharePoint document
/// libraries: no path separators or reserved characters, no reserved device
/// names, no trailing dots or spaces, bounded length.
pub fn safe_file_stem(text: &str) -> String {
    const MAX_CHARS: usize = 80;
    let cleaned: String =
        text.chars().map(|c| if c.is_control() || "<>:\"/\\|?*#%".contains(c) { ' ' } else { c }).collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(MAX_CHARS).collect();
    let trimmed = truncated.trim_end_matches(['.', ' ']).trim_start_matches(['.', ' ']).to_owned();
    let reserved = ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .map(|s| s.to_string())
        .chain((1..=9).flat_map(|n| [format!("COM{n}"), format!("LPT{n}")]))
        .any(|name| name.eq_ignore_ascii_case(trimmed.split('.').next().unwrap_or_default()));
    if trimmed.is_empty() || reserved {
        format!("_{trimmed}")
    } else {
        trimmed
    }
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
        assert!(fs::read_to_string(&path).unwrap().contains("\"schemaVersion\": 2"));
    }

    #[test]
    fn corrupt_and_foreign_files_are_reported_not_overwritten() {
        let path = temp_dir("json-corrupt").join("data.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(load_or_seed::<Vec<u32>>(&path, Vec::new).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");

        // PR #1 files had no schema version; a future version is also refused.
        for old in ["[1, 2]", r#"{"schemaVersion": 3, "data": []}"#] {
            fs::write(&path, old).unwrap();
            assert!(load_or_seed::<Vec<u32>>(&path, Vec::new).is_err(), "{old}");
            assert_eq!(fs::read_to_string(&path).unwrap(), old);
        }
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(safe_file_stem("056-2026 - בלט רומני"), "056-2026 - בלט רומני");
        assert_eq!(safe_file_stem("..\\..\\a/b: c?*"), "a b c");
        assert_eq!(safe_file_stem("  name.  "), "name");
        assert_eq!(safe_file_stem("CON"), "_CON");
        assert_eq!(safe_file_stem("com1.txt"), "_com1.txt");
        assert_eq!(safe_file_stem("///"), "_");
        assert_eq!(safe_file_stem(&"א".repeat(200)).chars().count(), 80);
    }
}
