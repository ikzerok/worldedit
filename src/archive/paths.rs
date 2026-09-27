use super::{Files, MANIFEST, MAX_BYTES, MAX_FILES};
use std::path::{Component, Path, PathBuf};
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveEntryKind {
    File,
    Directory,
}

pub(super) struct SeenArchivePath {
    key: String,
    path: PathBuf,
    kind: ArchiveEntryKind,
    components: Vec<String>,
}

pub fn relative_path(name: &str) -> Result<PathBuf, String> {
    let name = name.replace('\\', "/");
    let path = Path::new(&name);
    if name.is_empty()
        || name.contains(':')
        || name.starts_with('/')
        || name
            .split('/')
            .any(|part| part == ".." || part == "." || part.is_empty())
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("工程包包含不安全路径：{name}"));
    }
    Ok(path.to_owned())
}
/// 校验将被挂载或编码的完整相对文件集合。
///
/// 目录选择和 ZIP 编码必须共用这一边界，避免目录能导入、保存时才因
/// Windows 路径别名或事务目录失败。
pub fn validate_files(files: &Files) -> Result<(), String> {
    if files.len() > MAX_FILES || files.values().map(|v| v.len() as u64).sum::<u64>() > MAX_BYTES {
        return Err("工程包最多包含 4096 个文件、64 MiB 内容".into());
    }
    let mut seen = Vec::with_capacity(files.len());
    for path in files.keys() {
        let name = path.to_string_lossy().replace('\\', "/");
        let normalized = relative_path(&name)?;
        reject_transactions(&normalized)?;
        register_path(&mut seen, &normalized, ArchiveEntryKind::File)?;
    }
    Ok(())
}
/// 只在旧版入口清单缺失时补写默认记录；已有字节（包括未知字段）原样保留。
pub fn ensure_legacy_manifest(files: &mut Files, entry: &str) -> Result<(), String> {
    let entry = relative_path(entry)?;
    if !entry.extension().is_some_and(|extension| extension == "wl") || !files.contains_key(&entry)
    {
        return Err("工程包记录的 .wl 入口文件不存在".into());
    }
    if files.contains_key(Path::new(MANIFEST)) {
        if manifest_entry(files, Path::new(MANIFEST), true)?.as_ref() != Some(&entry) {
            return Err("工程包旧版入口记录与当前工程入口不一致".into());
        }
        return Ok(());
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "entry": entry.to_string_lossy().replace('\\', "/")
    }))
    .map_err(|error| error.to_string())?;
    files.insert(MANIFEST.into(), bytes);
    Ok(())
}
pub fn entry(files: &Files) -> Result<PathBuf, String> {
    let legacy = manifest_entry(files, Path::new(MANIFEST), true)?;
    // `.world/project.json` is authoring data.  Core owns its duplicate-key and
    // path semantics; the archive layer only asks core for the optional entry.
    let project = worldline_core::workspace_snapshot::project_entry(files)?;
    if let (Some(legacy), Some(project)) = (&legacy, &project) {
        if legacy != project {
            return Err("工程包含冲突的入口记录".into());
        }
    }
    if let Some(path) = legacy.or(project) {
        return Ok(path);
    }
    if files.contains_key(Path::new("world.wl")) {
        return Ok("world.wl".into());
    }
    let candidates: Vec<_> = files
        .keys()
        .filter(|p| p.extension().is_some_and(|e| e == "wl"))
        .collect();
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    Err("无法确定总入口：请在工程根目录提供 world.wl，再选择整个文件夹".into())
}
fn manifest_entry(files: &Files, path: &Path, required: bool) -> Result<Option<PathBuf>, String> {
    let Some(manifest) = files.get(path) else {
        return Ok(None);
    };
    let value: serde_json::Value = match serde_json::from_slice(manifest) {
        Ok(value) => value,
        Err(_error) if !required => return Ok(None),
        Err(error) => return Err(format!("工程入口记录无效：{error}")),
    };
    let Some(entry) = value.get("entry").and_then(serde_json::Value::as_str) else {
        if required {
            return Err("工程包缺少入口记录".into());
        }
        return Ok(None);
    };
    let entry = relative_path(entry)?;
    if !entry.extension().is_some_and(|extension| extension == "wl") || !files.contains_key(&entry)
    {
        return Err("工程包记录的 .wl 入口文件不存在".into());
    }
    Ok(Some(entry))
}
pub(super) fn reject_transactions(path: &Path) -> Result<(), String> {
    let components = path_components(path);
    if components
        .first()
        .is_some_and(|component| component.eq_ignore_ascii_case(".world"))
        && components
            .get(1)
            .is_some_and(|component| component.eq_ignore_ascii_case(".transactions"))
    {
        return Err("工程包不得包含未完成保存事务".into());
    }
    Ok(())
}
pub(super) fn register_path(
    seen: &mut Vec<SeenArchivePath>,
    path: &Path,
    kind: ArchiveEntryKind,
) -> Result<(), String> {
    let components = path_components(path);
    let key = components
        .iter()
        .map(|component| component.to_lowercase())
        .collect::<Vec<_>>()
        .join("/");
    for existing in seen.iter() {
        let same = key == existing.key;
        let existing_is_ancestor = key.starts_with(&format!("{}/", existing.key));
        let new_is_ancestor = existing.key.starts_with(&format!("{key}/"));
        let file_directory_conflict = (same && kind != existing.kind)
            || (existing.kind == ArchiveEntryKind::File && existing_is_ancestor)
            || (kind == ArchiveEntryKind::File && new_is_ancestor);
        let mut directory_spelling_conflict = false;
        for (component, existing_component) in components.iter().zip(existing.components.iter()) {
            if component.to_lowercase() != existing_component.to_lowercase() {
                break;
            }
            directory_spelling_conflict |= component != existing_component;
        }
        if same || file_directory_conflict || directory_spelling_conflict {
            return Err(format!(
                "工程包包含 Windows 不可区分的路径冲突：{} 与 {}",
                existing.path.display(),
                path.display()
            ));
        }
    }
    seen.push(SeenArchivePath {
        key,
        path: path.to_owned(),
        kind,
        components,
    });
    Ok(())
}

fn path_components(path: &Path) -> Vec<String> {
    path.to_string_lossy()
        .replace('\\', "/")
        .split('/')
        .map(str::to_owned)
        .collect()
}
