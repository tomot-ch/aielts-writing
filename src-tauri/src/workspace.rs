//! Desktop workspace lifecycle: command leases and verified directory migration.
use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct GateState {
    operations: usize,
    exclusive: Option<&'static str>,
}

#[derive(Clone, Default)]
pub struct WorkspaceGate(Arc<Mutex<GateState>>);

pub struct Lease {
    gate: WorkspaceGate,
    migration: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.0.lock() {
            if self.migration {
                state.exclusive = None;
            } else {
                state.operations -= 1;
            }
        }
    }
}
impl WorkspaceGate {
    pub fn operation(&self) -> Result<Lease, String> {
        let mut state = self.0.lock().map_err(|_| "数据目录状态锁失败")?;
        if let Some(action) = state.exclusive {
            return Err(format!("正在{action}，请稍后重试"));
        }
        state.operations += 1;
        Ok(Lease {
            gate: self.clone(),
            migration: false,
        })
    }
    pub fn migration(&self) -> Result<Lease, String> {
        self.exclusive("迁移数据目录")
    }
    pub fn exclusive(&self, action: &'static str) -> Result<Lease, String> {
        let mut state = self.0.lock().map_err(|_| "数据目录状态锁失败")?;
        if state.exclusive.is_some() || state.operations > 0 {
            return Err(format!("有保存或模型请求正在进行，请完成后再{action}"));
        }
        state.exclusive = Some(action);
        Ok(Lease {
            gate: self.clone(),
            migration: true,
        })
    }
}

pub struct ServiceLease<T> {
    pub service: T,
    pub _lease: Lease,
}
impl<T> Deref for ServiceLease<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.service
    }
}

pub fn canonical_target(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("请选择绝对路径".into());
    }
    if path.exists() {
        if fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("数据目录不能是符号链接，请选择实际目录".into());
        }
        if !path.is_dir() {
            return Err("请选择文件夹".into());
        }
        return fs::canonicalize(path).map_err(|e| e.to_string());
    }
    let name = path.file_name().ok_or("目标目录名称无效")?;
    let parent =
        fs::canonicalize(path.parent().ok_or("目标目录无父目录")?).map_err(|e| e.to_string())?;
    Ok(parent.join(name))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let output = target.join(entry.file_name());
        if kind.is_symlink() {
            return Err("数据目录含符号链接，请先移除链接后再迁移".into());
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &output)?;
        } else if kind.is_file() {
            let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            if fs::read(&output).map_err(|e| e.to_string())? != bytes {
                return Err("迁移校验失败".into());
            }
        } else {
            return Err("数据目录含不支持的文件类型".into());
        }
    }
    Ok(())
}

/// The caller must hold the exclusive workspace lease and ensure AI jobs are idle.
/// Only an empty/new target is accepted. Failure keeps the old workspace active.
pub fn migrate<T>(
    source: &Path,
    target: &Path,
    activate: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let source = fs::canonicalize(source).map_err(|e| e.to_string())?;
    let target = canonical_target(target)?;
    if target.starts_with(&source) || source.starts_with(&target) {
        return Err("目标目录不能与旧目录互相包含".into());
    }
    let had_target = target.exists();
    if had_target
        && fs::read_dir(&target)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("迁移目标必须是空文件夹，请选择新的或空的目录".into());
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = target
        .parent()
        .unwrap()
        .join(format!(".ielts-migration-{}-{unique}", std::process::id()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let copied = copy_tree(&source, &staging);
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let published = (|| {
        if had_target {
            fs::remove_dir(&target).map_err(|e| e.to_string())?;
        }
        fs::rename(&staging, &target).map_err(|e| e.to_string())
    })();
    if let Err(error) = published {
        let _ = fs::remove_dir_all(&staging);
        if had_target {
            let _ = fs::create_dir(&target);
        }
        return Err(error);
    }
    match activate(&target) {
        Ok(value) => Ok(value),
        Err(error) => {
            // This target contains only the snapshot created by this operation.
            if let Err(cleanup) = fs::remove_dir_all(&target) {
                return Err(format!(
                    "{error}；旧目录仍在使用，迁移副本清理失败：{cleanup}"
                ));
            }
            if had_target {
                fs::create_dir(&target).map_err(|e| e.to_string())?;
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_excludes_commands_and_releases_after_errors() {
        let gate = WorkspaceGate::default();
        let operation = gate.operation().unwrap();
        assert!(gate.migration().is_err());
        drop(operation);
        let migration = gate.migration().unwrap();
        assert!(gate.operation().is_err());
        assert!(gate.migration().is_err());
        drop(migration);
        assert!(gate.operation().is_ok());
    }
    #[test]
    fn migration_rolls_back_failed_activation_and_preserves_source() {
        let root = std::env::temp_dir().join(format!(
            "migration-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("old");
        let target = root.join("new");
        fs::create_dir_all(source.join("essay")).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(source.join("essay/draft.json"), b"essay contents").unwrap();
        assert!(migrate::<()>(&source, &target, |_| Err(
            "configuration write failed".into()
        ))
        .is_err());
        assert_eq!(
            fs::read(source.join("essay/draft.json")).unwrap(),
            b"essay contents"
        );
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        migrate(&source, &target, |path| {
            assert_eq!(
                fs::read(path.join("essay/draft.json")).unwrap(),
                b"essay contents"
            );
            Ok(())
        })
        .unwrap();
        assert!(source.exists());
        assert!(target.join("essay/draft.json").exists());
        assert!(migrate(&source, &target, |_| Ok(())).is_err());
        assert!(migrate(&source, &source.join("nested"), |_| Ok(())).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn migration_rejects_symlinks_without_following_them() {
        let root = std::env::temp_dir().join(format!(
            "migration-symlink-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("old");
        let target = root.join("new");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&source, source.join("recursive-link")).unwrap();
        assert!(migrate(&source, &target, |_| Ok(())).is_err());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        assert!(source.join("recursive-link").is_symlink());
        std::os::unix::fs::symlink(&source, root.join("alias")).unwrap();
        assert!(canonical_target(&root.join("alias")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
