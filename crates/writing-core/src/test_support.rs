use crate::{infrastructure::FileWorkspace, ports::WorkspaceStore, Result};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Default)]
pub(crate) struct FailOnceStore {
    failure: Mutex<Option<(String, Option<String>)>>,
    rename_failure: Mutex<bool>,
}
impl FailOnceStore {
    pub fn fail_next(&self, suffix: &str) {
        *self.failure.lock().unwrap() = Some((suffix.into(), None));
    }
    pub fn fail_when(&self, suffix: &str, contents: &str) {
        *self.failure.lock().unwrap() = Some((suffix.into(), Some(contents.into())));
    }
    pub fn fail_next_rename(&self) {
        *self.rename_failure.lock().unwrap() = true;
    }
}
impl WorkspaceStore for FailOnceStore {
    fn create_dir(&self, path: &Path) -> Result<()> {
        FileWorkspace.create_dir(path)
    }
    fn children(&self, path: &Path) -> Result<Vec<PathBuf>> {
        FileWorkspace.children(path)
    }
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        FileWorkspace.read_bytes(path)
    }
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let mut failure = self.failure.lock().unwrap();
        if failure.as_ref().is_some_and(|(suffix, contents)| {
            path.to_string_lossy().ends_with(suffix)
                && contents
                    .as_ref()
                    .is_none_or(|text| String::from_utf8_lossy(bytes).contains(text))
        }) {
            *failure = None;
            return Err("injected disk write failure".into());
        }
        drop(failure);
        FileWorkspace.write_atomic(path, bytes)
    }
    fn exists(&self, path: &Path) -> bool {
        FileWorkspace.exists(path)
    }
    fn rename(&self, source: &Path, target: &Path) -> Result<()> {
        if std::mem::take(&mut *self.rename_failure.lock().unwrap()) {
            return Err("injected directory move failure".into());
        }
        FileWorkspace.rename(source, target)
    }
    fn remove_file(&self, path: &Path) -> Result<()> {
        FileWorkspace.remove_file(path)
    }
}
