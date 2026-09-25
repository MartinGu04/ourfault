//! Exported files go to a fixed local folder (by default
//! `Downloads\OurFault`). The webview never supplies a path.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{AdapterError, ExportSink, ExportedFile};

pub struct LocalFolderExport {
    folder: PathBuf,
}

impl LocalFolderExport {
    pub fn new(folder: PathBuf) -> Self {
        Self { folder }
    }
}

impl ExportSink for LocalFolderExport {
    fn write(&self, file_name: &str, bytes: &[u8]) -> Result<ExportedFile, AdapterError> {
        if file_name.contains(['/', '\\']) || file_name.starts_with('.') {
            return Err(AdapterError::Storage(format!("unsafe export file name {file_name:?}")));
        }
        fs::create_dir_all(&self.folder)?;
        let (stem, extension) = file_name.rsplit_once('.').unwrap_or((file_name, ""));
        for attempt in 1..1000 {
            let name = match (attempt, extension) {
                (1, _) => file_name.to_owned(),
                (n, "") => format!("{stem} ({n})"),
                (n, ext) => format!("{stem} ({n}).{ext}"),
            };
            // `create_new` never overwrites an existing file.
            match OpenOptions::new().write(true).create_new(true).open(self.folder.join(&name)) {
                Ok(mut file) => {
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    return Ok(ExportedFile { file_name: name, folder: display(&self.folder) });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(AdapterError::Storage("no free export file name".into()))
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;

    #[test]
    fn never_overwrites_existing_files() {
        let folder = temp_dir("export");
        let sink = LocalFolderExport::new(folder.clone());
        assert_eq!(sink.write("a.pdf", b"1").unwrap().file_name, "a.pdf");
        assert_eq!(sink.write("a.pdf", b"2").unwrap().file_name, "a (2).pdf");
        assert_eq!(fs::read(folder.join("a.pdf")).unwrap(), b"1");
        assert!(sink.write("../x.pdf", b"3").is_err());
    }
}
