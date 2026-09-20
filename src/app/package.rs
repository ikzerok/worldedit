//! 桌面 ZIP 工程包导出：沿用 core 的严格导出，写入新文件且不覆盖已有作品。

use super::WorldeditApp;
use crate::archive::{self, Files};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use worldline_core::project::Project;

/// 生成桌面与浏览器可互读的严格工程包。
pub(crate) fn export_package_bytes(project: &Project) -> Result<Vec<u8>, String> {
    let mut files: Files = project.export_files()?.into_iter().collect();
    let entry = project
        .entry
        .strip_prefix(&project.root)
        .map_err(|_| "总入口不在工程目录内")?
        .to_string_lossy()
        .replace('\\', "/");
    archive::ensure_legacy_manifest(&mut files, &entry)?;
    archive::entry(&files)?;
    archive::encode(&files)
}

/// 在目标不存在时写入完整 ZIP；目标已存在或位于当前工作区内都会拒绝。
pub(crate) fn write_package_file(
    workspace_root: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let root = normalized_path(workspace_root);
    let target = normalized_path(target);
    if is_same_or_descendant(&root, &target) {
        return Err("工程包目标必须在当前工作区外".into());
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("目标 ZIP 已存在，未覆盖：{}", target.display())
            } else {
                format!("无法创建目标 ZIP：{}：{error}", target.display())
            }
        })?;
    let result = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let cleanup = fs::remove_file(&target);
        return Err(match cleanup {
            Ok(()) => format!("写入目标 ZIP 失败：{error}"),
            Err(cleanup_error) => {
                format!("写入目标 ZIP 失败：{error}；清理不完整文件也失败：{cleanup_error}")
            }
        });
    }
    Ok(())
}

fn is_same_or_descendant(root: &Path, target: &Path) -> bool {
    let mut root_components = root.components();
    let mut target_components = target.components();
    loop {
        let Some(root_component) = root_components.next() else {
            return true;
        };
        let Some(target_component) = target_components.next() else {
            return false;
        };
        if !same_component(root_component.as_os_str(), target_component.as_os_str()) {
            return false;
        }
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let canonical = absolute.canonicalize().unwrap_or_else(|_| {
        match (absolute.parent(), absolute.file_name()) {
            (Some(parent), Some(name)) if parent != absolute => normalized_path(parent).join(name),
            _ => absolute.clone(),
        }
    });
    let mut normalized = std::path::PathBuf::new();
    for component in canonical.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn same_component(left: &OsStr, right: &OsStr) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

impl WorldeditApp {
    pub(super) fn export_package(&mut self) {
        let parent = self.project.root.parent().unwrap_or(Path::new("."));
        let stem = self
            .project
            .root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "world-project".into());
        let Some(target) = rfd::FileDialog::new()
            .set_directory(parent)
            .set_file_name(format!("{stem}-export.zip"))
            .add_filter("ZIP 工程包", &["zip"])
            .save_file()
        else {
            return;
        };
        match export_package_bytes(&self.project)
            .and_then(|bytes| write_package_file(&self.project.root, &target, &bytes))
        {
            Ok(()) => {
                self.io_error = None;
                self.message = Some("严格工程已导出为 ZIP（未改变当前保存状态）".into());
            }
            Err(error) => self.io_error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("worldedit-package-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn package_file_roundtrips_exact_bytes() {
        let root = test_root("roundtrip");
        let target = root.with_file_name(format!(
            "{}-export.zip",
            root.file_name().unwrap().to_string_lossy()
        ));
        let bytes = b"PK\x03\x04 opaque package bytes\0\xff";
        write_package_file(&root, &target, bytes).unwrap();
        assert_eq!(fs::read(&target).unwrap(), bytes);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_file(target);
    }

    #[test]
    fn package_file_rejects_existing_target_without_changing_it() {
        let root = test_root("existing");
        let target = root.with_file_name(format!(
            "{}-export.zip",
            root.file_name().unwrap().to_string_lossy()
        ));
        let original = b"keep this package";
        fs::write(&target, original).unwrap();
        let error = write_package_file(&root, &target, b"replace me").unwrap_err();
        assert!(error.contains("已存在"));
        assert_eq!(fs::read(&target).unwrap(), original);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_file(target);
    }

    #[test]
    fn package_file_rejects_workspace_target_and_failed_parent_without_file() {
        let root = test_root("失败-工作区");
        let inside = root.join(".").join("nested.zip");
        let error = write_package_file(&root, &inside, b"must not write").unwrap_err();
        assert!(error.contains("工作区外"));
        assert!(!inside.exists());

        let missing_parent = root
            .with_file_name(format!(
                "{}-missing",
                root.file_name().unwrap().to_string_lossy()
            ))
            .join("nested.zip");
        let error = write_package_file(&root, &missing_parent, b"must fail").unwrap_err();
        assert!(error.contains("无法创建目标 ZIP"));
        assert!(!missing_parent.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn normalized_path_keeps_unc_verbatim_prefix() {
        let path = Path::new(r"\\?\UNC\server\share\workspace\export.zip");
        let normalized = normalized_path(path);
        assert!(normalized
            .to_string_lossy()
            .starts_with(r"\\?\UNC\server\share\workspace"));
    }

    #[test]
    fn strict_export_package_roundtrips_through_archive() {
        let root = test_root("content");
        let project = Project::new(&root);
        let bytes = export_package_bytes(&project).unwrap();
        let files = archive::decode(&bytes).unwrap();
        assert_eq!(archive::entry(&files).unwrap(), Path::new("world.wl"));
        assert_eq!(
            files[Path::new("world.wl")],
            project.documents[&project.entry].text.as_bytes()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn strict_export_rejects_legacy_entry_that_differs_from_project_entry() {
        let root = test_root("legacy-entry-conflict");
        fs::write(
            root.join("worldedit-project.json"),
            br#"{"entry":"other.wl","future":true}"#,
        )
        .unwrap();
        fs::write(root.join("other.wl"), b"event other\n  -> END\n").unwrap();
        let project = Project::new(&root);
        let error = export_package_bytes(&project).unwrap_err();
        assert!(error.contains("旧版入口记录与当前工程入口不一致"));
        let _ = fs::remove_dir_all(root);
    }
}
