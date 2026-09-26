//! 浏览器工程包：保留目录与二进制素材，限制展开大小，拒绝越界路径。
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

pub type Files = BTreeMap<PathBuf, Vec<u8>>;
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 4096;
pub const MANIFEST: &str = "worldedit-project.json";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64: usize = 4 * 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const MAX_BROWSER_SNAPSHOT_ENVELOPE_BYTES: usize =
    MAX_BYTES as usize * 4 / 3 + MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 + 16 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MAX_BROWSER_RECOVERY_BYTES: u64 =
    MAX_BYTES + (MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 as u64 * 3 / 4) + 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_MANIFEST: &str = "worldedit-recovery.json";
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_PROJECT: &str = "worldedit-project.zip";
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_CHECKPOINTS: &str = "worldedit-checkpoints.bin";

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct BrowserRecoveryBundle {
    pub(crate) project_files: Files,
    pub(crate) checkpoint_session_id: String,
    pub(crate) checkpoint_snapshot: Vec<u8>,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserRecoveryManifest {
    format: String,
    version: u32,
    checkpoint_session_id: String,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DecodedBrowserSnapshot {
    pub(crate) archive_base64: String,
    pub(crate) checkpoint_session_id: Option<String>,
    pub(crate) checkpoint_snapshot_base64: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredBrowserProject {
    version: u32,
    checkpoint_session_id: String,
    archive_base64: String,
    #[serde(default)]
    checkpoint_snapshot_base64: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn encode_browser_snapshot(
    archive_base64: &str,
    checkpoint_session_id: &str,
    checkpoint_snapshot_base64: Option<&str>,
) -> Result<String, String> {
    if archive_base64.len() > (MAX_BYTES as usize * 4 / 3 + 8) {
        return Err("浏览器工程快照超过 64 MiB 包体上限".into());
    }
    if checkpoint_snapshot_base64
        .is_some_and(|snapshot| snapshot.len() > MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64)
    {
        return Err("浏览器检查点历史超过本地快照上限".into());
    }
    serde_json::to_string(&StoredBrowserProject {
        version: 2,
        checkpoint_session_id: checkpoint_session_id.to_owned(),
        archive_base64: archive_base64.to_owned(),
        checkpoint_snapshot_base64: checkpoint_snapshot_base64.map(str::to_owned),
    })
    .map_err(|error| format!("无法编码浏览器工程快照：{error}"))
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn decode_browser_snapshot(stored: &str) -> Result<DecodedBrowserSnapshot, String> {
    if !stored.trim_start().starts_with('{') {
        // The original localStorage format stored the ZIP as a bare base64 string.
        if stored.len() > (MAX_BYTES as usize * 4 / 3 + 8) {
            return Err("浏览器工程快照超过 64 MiB 包体上限".into());
        }
        return Ok(DecodedBrowserSnapshot {
            archive_base64: stored.to_owned(),
            checkpoint_session_id: None,
            checkpoint_snapshot_base64: None,
        });
    }
    if stored.len() > MAX_BROWSER_SNAPSHOT_ENVELOPE_BYTES {
        return Err("浏览器工程快照超过存储上限".into());
    }
    let snapshot: StoredBrowserProject =
        serde_json::from_str(stored).map_err(|error| format!("浏览器工程快照格式无效：{error}"))?;
    if !matches!(snapshot.version, 1 | 2) {
        return Err("浏览器工程快照版本不受支持".into());
    }
    if !valid_checkpoint_session_id(&snapshot.checkpoint_session_id) {
        return Err("浏览器工程快照中的检查点会话标识无效".into());
    }
    if snapshot.archive_base64.len() > (MAX_BYTES as usize * 4 / 3 + 8)
        || snapshot
            .checkpoint_snapshot_base64
            .as_ref()
            .is_some_and(|value| value.len() > MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64)
    {
        return Err("浏览器工程快照超过存储上限".into());
    }
    Ok(DecodedBrowserSnapshot {
        archive_base64: snapshot.archive_base64,
        checkpoint_session_id: Some(snapshot.checkpoint_session_id),
        checkpoint_snapshot_base64: snapshot.checkpoint_snapshot_base64,
    })
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn encode_browser_recovery_bundle(
    project_archive: &[u8],
    checkpoint_snapshot: &[u8],
    checkpoint_session_id: &str,
) -> Result<Vec<u8>, String> {
    if project_archive.len() as u64 > MAX_BYTES
        || checkpoint_snapshot.len() > MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 * 3 / 4
        || !valid_checkpoint_session_id(checkpoint_session_id)
    {
        return Err("浏览器恢复副本超过大小上限或会话标识无效".into());
    }
    let manifest = serde_json::to_vec(&BrowserRecoveryManifest {
        format: "worldedit-recovery".into(),
        version: 1,
        checkpoint_session_id: checkpoint_session_id.to_owned(),
    })
    .map_err(|error| format!("无法编码浏览器恢复副本清单：{error}"))?;
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        (RECOVERY_MANIFEST, manifest.as_slice()),
        (RECOVERY_PROJECT, project_archive),
        (RECOVERY_CHECKPOINTS, checkpoint_snapshot),
    ] {
        archive
            .start_file(name, options)
            .map_err(|error| error.to_string())?;
        archive
            .write_all(bytes)
            .map_err(|error| error.to_string())?;
    }
    let bytes = archive
        .finish()
        .map_err(|error| error.to_string())?
        .into_inner();
    if bytes.len() as u64 > MAX_BROWSER_RECOVERY_BYTES {
        return Err("浏览器恢复副本超过大小上限".into());
    }
    Ok(bytes)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn decode_browser_recovery_bundle(
    bytes: &[u8],
) -> Result<Option<BrowserRecoveryBundle>, String> {
    if bytes.len() as u64 > MAX_BROWSER_RECOVERY_BYTES {
        return Err("浏览器恢复副本超过大小上限".into());
    }
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Ok(None);
    };
    let manifest_bytes = match archive.by_name(RECOVERY_MANIFEST) {
        Ok(manifest) => {
            if manifest.size() > 4096 {
                return Err("浏览器恢复副本清单超过大小上限".into());
            }
            let mut bytes = Vec::new();
            manifest
                .take(4097)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            bytes
        }
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let manifest: BrowserRecoveryManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("浏览器恢复副本清单无效：{error}"))?;
    if manifest.format != "worldedit-recovery" || manifest.version != 1 {
        return Err("浏览器恢复副本格式或版本不受支持".into());
    }
    if !valid_checkpoint_session_id(&manifest.checkpoint_session_id) || archive.len() != 3 {
        return Err("浏览器恢复副本会话标识或文件数无效".into());
    }
    let names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .map(|entry| entry.name().to_owned())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if names
        .iter()
        .any(|name| name.contains('/') || name.contains('\\'))
        || !names.iter().any(|name| name == RECOVERY_MANIFEST)
        || !names.iter().any(|name| name == RECOVERY_PROJECT)
        || !names.iter().any(|name| name == RECOVERY_CHECKPOINTS)
    {
        return Err("浏览器恢复副本包含意外文件".into());
    }
    let project_archive = read_recovery_entry(&mut archive, RECOVERY_PROJECT, MAX_BYTES)?;
    let checkpoint_snapshot = read_recovery_entry(
        &mut archive,
        RECOVERY_CHECKPOINTS,
        (MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 * 3 / 4) as u64,
    )?;
    let project_files = decode(&project_archive)?;
    entry(&project_files)?;
    Ok(Some(BrowserRecoveryBundle {
        project_files,
        checkpoint_session_id: manifest.checkpoint_session_id,
        checkpoint_snapshot,
    }))
}

#[cfg(any(target_arch = "wasm32", test))]
fn read_recovery_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let file = archive.by_name(name).map_err(|error| error.to_string())?;
    if file.size() > max_bytes {
        return Err("浏览器恢复副本负载超过大小上限".into());
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err("浏览器恢复副本负载超过大小上限".into());
    }
    Ok(bytes)
}

#[cfg(any(target_arch = "wasm32", test))]
fn valid_checkpoint_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArchiveEntryKind {
    File,
    Directory,
}

struct SeenArchivePath {
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

pub fn encode(files: &Files) -> Result<Vec<u8>, String> {
    validate_files(files)?;
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, bytes) in files {
        let name = path.to_string_lossy().replace('\\', "/");
        archive
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        archive.write_all(bytes).map_err(|e| e.to_string())?;
    }
    let bytes = archive
        .finish()
        .map(|out| out.into_inner())
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("压缩后的工程包超过 64 MiB".into());
    }
    Ok(bytes)
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

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn decode(bytes: &[u8]) -> Result<Files, String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("工程包超过 64 MiB".into());
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("工程包无法读取：{e}"))?;
    if archive.len() > MAX_FILES {
        return Err("工程包文件数量超过 4096".into());
    }
    let mut files = Files::new();
    let mut seen = Vec::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|e| e.to_string())?;
        let is_directory = file.is_dir()
            || file
                .name()
                .chars()
                .last()
                .is_some_and(|character| matches!(character, '/' | '\\'));
        let name = if is_directory {
            file.name()
                .strip_suffix('/')
                .or_else(|| file.name().strip_suffix('\\'))
                .unwrap_or(file.name())
        } else {
            file.name()
        };
        let path = relative_path(name)?;
        reject_transactions(&path)?;
        register_path(
            &mut seen,
            &path,
            if is_directory {
                ArchiveEntryKind::Directory
            } else {
                ArchiveEntryKind::File
            },
        )?;
        if is_directory {
            continue;
        }
        if file.size() > MAX_BYTES - total {
            return Err("工程包展开后超过 64 MiB".into());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_BYTES - total + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len() as u64;
        if total > MAX_BYTES {
            return Err("工程包展开后超过 64 MiB".into());
        }
        if files.insert(path, bytes).is_some() {
            return Err("工程包含重复文件路径".into());
        }
    }
    validate_files(&files)?;
    Ok(files)
}

/// 预检一次即将打开的浏览器工作区，并返回可保存的完整文件集合。
///
/// 旧入口清单在这里补入后再执行原始大小、文件数和最终 ZIP 大小检查，
/// 因而目录导入不会先成功、等到第一次保存才发现包不可写。调用方应保留
/// 返回的文件集合；附件选择等非工作区打开操作不应调用此函数。
#[cfg(any(target_arch = "wasm32", test))]
pub fn prepare_import(mut files: Files) -> Result<Files, String> {
    if files.len() == 1 {
        let (name, bytes) = files.first_key_value().unwrap();
        if name
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        {
            files = decode(bytes)?;
        }
    }
    let entry_name = entry(&files)?;
    let entry_name = entry_name.to_string_lossy().replace('\\', "/");
    ensure_legacy_manifest(&mut files, &entry_name)?;
    entry(&files)?;
    // 导入时只在原始边界通过后编码一次，以确认最终 ZIP 上限；保存时若
    // 缓冲已经改变，则必须对当前快照重新编码。
    encode(&files)?;
    Ok(files)
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

fn reject_transactions(path: &Path) -> Result<(), String> {
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

fn register_path(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_roundtrip_preserves_nested_sources_assets_and_entry() {
        let files = Files::from([
            (
                "world.wl".into(),
                "include \"events/雾港.wl\"\n".as_bytes().to_vec(),
            ),
            (
                "events/雾港.wl".into(),
                "event start\n  你好。\n  -> END\n".as_bytes().to_vec(),
            ),
            ("assets/image.png".into(), vec![0, 128, 255, 1]),
        ]);
        let restored = decode(&encode(&files).unwrap()).unwrap();
        assert_eq!(restored, files);
        assert_eq!(entry(&restored).unwrap(), Path::new("world.wl"));
        for path in [
            "../bad.wl",
            "/bad.wl",
            "C:/bad.wl",
            "events/../../bad.wl",
            "..\\bad.wl",
        ] {
            assert!(relative_path(path).is_err(), "{path}");
        }
    }

    #[test]
    fn browser_snapshot_roundtrip_keeps_checkpoint_session_and_reads_legacy_base64() {
        let stored =
            encode_browser_snapshot("UEsDBA==", "browser-session-a", Some("d2xjcA==")).unwrap();
        assert_eq!(
            decode_browser_snapshot(&stored).unwrap(),
            DecodedBrowserSnapshot {
                archive_base64: "UEsDBA==".into(),
                checkpoint_session_id: Some("browser-session-a".into()),
                checkpoint_snapshot_base64: Some("d2xjcA==".into()),
            }
        );
        let legacy_envelope = decode_browser_snapshot(
            r#"{"version":1,"checkpoint_session_id":"browser-session-a","archive_base64":"UEsDBA=="}"#,
        )
        .unwrap();
        assert_eq!(
            legacy_envelope.checkpoint_session_id.as_deref(),
            Some("browser-session-a")
        );
        assert_eq!(legacy_envelope.checkpoint_snapshot_base64, None);
        assert_eq!(
            decode_browser_snapshot("UEsDBA==").unwrap(),
            DecodedBrowserSnapshot {
                archive_base64: "UEsDBA==".into(),
                checkpoint_session_id: None,
                checkpoint_snapshot_base64: None,
            }
        );
        assert!(decode_browser_snapshot(
            r#"{"version":3,"checkpoint_session_id":"browser-session-a","archive_base64":"UEsDBA=="}"#
        )
        .is_err());
        assert!(decode_browser_snapshot(
            r#"{"version":1,"checkpoint_session_id":"../other","archive_base64":"UEsDBA=="}"#
        )
        .is_err());
        assert!(encode_browser_snapshot(
            "UEsDBA==",
            "browser-session-a",
            Some(&"x".repeat(MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 + 1)),
        )
        .is_err());
    }

    #[test]
    fn browser_recovery_bundle_roundtrips_workspace_and_checkpoint_payload() {
        let project_files = Files::from([("world.wl".into(), b"event start\n".to_vec())]);
        let project_archive = encode(&prepare_import(project_files).unwrap()).unwrap();
        let checkpoints = b"bounded checkpoint bytes";
        let recovery =
            encode_browser_recovery_bundle(&project_archive, checkpoints, "browser-session-a")
                .unwrap();
        let decoded = decode_browser_recovery_bundle(&recovery).unwrap().unwrap();
        assert_eq!(decoded.project_files, decode(&project_archive).unwrap());
        assert_eq!(decoded.checkpoint_session_id, "browser-session-a");
        assert_eq!(decoded.checkpoint_snapshot, checkpoints);
        assert!(decode_browser_recovery_bundle(&project_archive)
            .unwrap()
            .is_none());
    }

    #[test]
    fn archive_roundtrip_preserves_authoring_manifest_and_opaque_json_bytes() {
        let files = Files::from([
            ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
            (
                MANIFEST.into(),
                br#"{"entry":"world.wl","future":true}"#.to_vec(),
            ),
            (
                ".world/project.json".into(),
                br#"{"schema_version":1,"unknown":{"kept":true},"entry":"world.wl"}"#.to_vec(),
            ),
            (".world/maps/raw.json".into(), vec![b'{', 0xff, b'}']),
            ("assets/map.png".into(), vec![0, 1, 2, 255]),
        ]);
        let restored = decode(&encode(&files).unwrap()).unwrap();
        assert_eq!(restored, files);
        assert_eq!(entry(&restored).unwrap(), Path::new("world.wl"));
    }

    #[test]
    fn entry_rejects_conflicting_legacy_and_project_manifests() {
        let files = Files::from([
            ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
            (MANIFEST.into(), br#"{"entry":"world.wl"}"#.to_vec()),
            (
                ".world/project.json".into(),
                br#"{"schema_version":1,"entry":"other.wl"}"#.to_vec(),
            ),
            ("other.wl".into(), b"event other\n  -> END\n".to_vec()),
        ]);
        assert!(entry(&files).is_err());
    }

    #[test]
    fn entry_uses_project_manifest_when_legacy_record_is_absent() {
        let files = Files::from([
            (
                "stories/intro.wl".into(),
                b"event start\n  -> END\n".to_vec(),
            ),
            (
                ".world/project.json".into(),
                br#"{"schema_version":1,"entry":"stories/intro.wl"}"#.to_vec(),
            ),
        ]);
        assert_eq!(entry(&files).unwrap(), Path::new("stories/intro.wl"));
    }

    #[test]
    fn entry_does_not_use_last_duplicate_project_manifest_key() {
        let files = Files::from([
            ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
            ("other.wl".into(), b"event other\n  -> END\n".to_vec()),
            (
                ".world/project.json".into(),
                br#"{"entry":"other.wl","entry":"world.wl"}"#.to_vec(),
            ),
        ]);
        assert_eq!(entry(&files).unwrap(), Path::new("world.wl"));
    }

    #[test]
    fn legacy_manifest_is_preserved_and_missing_entry_is_added() {
        let legacy = br#"{"entry":"world.wl","future":{"kept":true}}"#.to_vec();
        let mut files = Files::from([
            ("world.wl".into(), b"event start".to_vec()),
            (MANIFEST.into(), legacy.clone()),
            (
                ".world/project.json".into(),
                br#"{"schema_version":1,"entry":"world.wl"}"#.to_vec(),
            ),
        ]);
        ensure_legacy_manifest(&mut files, "world.wl").unwrap();
        assert_eq!(files[Path::new(MANIFEST)], legacy);
        assert_eq!(entry(&files).unwrap(), Path::new("world.wl"));

        let conflict_legacy = br#"{"entry":"other.wl","future":{"kept":true}}"#.to_vec();
        let mut conflict = Files::from([
            ("world.wl".into(), b"event start".to_vec()),
            ("other.wl".into(), b"event other".to_vec()),
            (MANIFEST.into(), conflict_legacy.clone()),
            (
                ".world/project.json".into(),
                br#"{"schema_version":1,"entry":"world.wl"}"#.to_vec(),
            ),
        ]);
        assert!(ensure_legacy_manifest(&mut conflict, "world.wl").is_err());
        assert_eq!(conflict[Path::new(MANIFEST)], conflict_legacy);
        assert!(entry(&conflict).is_err());

        let mut legacy_only = Files::from([
            ("world.wl".into(), b"event start".to_vec()),
            ("other.wl".into(), b"event other".to_vec()),
            (
                MANIFEST.into(),
                br#"{"entry":"other.wl","future":true}"#.to_vec(),
            ),
        ]);
        assert!(ensure_legacy_manifest(&mut legacy_only, "world.wl").is_err());
        assert_eq!(
            legacy_only[Path::new(MANIFEST)],
            br#"{"entry":"other.wl","future":true}"#
        );

        let mut missing = Files::from([("world.wl".into(), b"event start".to_vec())]);
        ensure_legacy_manifest(&mut missing, "world.wl").unwrap();
        assert_eq!(entry(&missing).unwrap(), Path::new("world.wl"));
    }

    #[test]
    fn archive_rejects_transaction_directory() {
        let files = Files::from([(
            ".world/.transactions/tx/journal.json".into(),
            b"{}".to_vec(),
        )]);
        assert!(encode(&files).is_err());
        let mixed_case = Files::from([(
            ".WORLD/.TRANSACTIONS/tx/journal.json".into(),
            b"{}".to_vec(),
        )]);
        assert!(encode(&mixed_case).is_err());
        let windows_style = Files::from([(
            r#".world\.transactions\tx\journal.json"#.into(),
            b"{}".to_vec(),
        )]);
        assert!(encode(&windows_style).is_err());

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            ".world/.transactions/tx/journal.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"{}").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            ".WORLD/.TRANSACTIONS/tx/journal.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"{}").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn archive_rejects_windows_case_collisions_on_encode_and_decode() {
        let files = Files::from([
            ("Maps/Overview.wl".into(), b"one".to_vec()),
            ("maps/overview.wl".into(), b"two".to_vec()),
        ]);
        assert!(encode(&files).is_err());

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in [("Maps/Overview.wl", b"one"), (r"maps\overview.wl", b"two")] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn archive_rejects_directory_and_file_collisions_after_normalization() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_directory("Maps/", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.start_file("maps", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"file").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());

        let files = Files::from([
            ("Assets/a.txt".into(), b"one".to_vec()),
            ("assets/b.txt".into(), b"two".to_vec()),
        ]);
        assert!(encode(&files).is_err());
        let ordinary = Files::from([
            ("assets/a.txt".into(), b"one".to_vec()),
            ("assets/b.txt".into(), b"two".to_vec()),
        ]);
        assert!(encode(&ordinary).is_ok());
        let different_parent = Files::from([
            ("assets/A.txt".into(), b"one".to_vec()),
            ("notes/a.txt".into(), b"two".to_vec()),
        ]);
        assert!(encode(&different_parent).is_ok());

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_directory("Maps/", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.add_directory("maps\\", zip::write::SimpleFileOptions::default())
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in [("Assets/a.txt", b"one"), ("assets/b.txt", b"two")] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn validate_files_rejects_paths_that_directory_import_must_reject() {
        let files = Files::from([
            ("Assets/a.txt".into(), b"one".to_vec()),
            ("assets/b.txt".into(), b"two".to_vec()),
        ]);
        assert!(validate_files(&files).is_err());

        let transactions = Files::from([(
            ".WORLD/.TRANSACTIONS/tx/journal.json".into(),
            b"{}".to_vec(),
        )]);
        assert!(validate_files(&transactions).is_err());
    }

    #[test]
    fn raw_import_boundary_can_fail_after_legacy_manifest_is_added() {
        let mut files = Files::new();
        files.insert("world.wl".into(), b"event start".to_vec());
        for index in 0..(4096 - 1) {
            files.insert(format!("assets/{index}.bin").into(), vec![index as u8]);
        }
        assert_eq!(files.len(), 4096);
        assert!(validate_files(&files).is_ok());
        assert!(prepare_import(files).is_err());
    }

    #[test]
    fn raw_size_boundary_can_fail_after_legacy_manifest_is_added() {
        let mut files = Files::new();
        files.insert("world.wl".into(), b"w".to_vec());
        files.insert(
            "opaque.bin".into(),
            vec![0_u8; MAX_BYTES as usize - files[Path::new("world.wl")].len()],
        );
        assert_eq!(
            files.values().map(|bytes| bytes.len() as u64).sum::<u64>(),
            MAX_BYTES
        );
        assert!(validate_files(&files).is_ok());
        assert!(prepare_import(files).is_err());
    }

    #[test]
    fn prepare_import_returns_the_same_complete_files_that_save_will_encode() {
        let raw = Files::from([("world.wl".into(), b"event start".to_vec())]);
        assert!(!raw.contains_key(Path::new(MANIFEST)));
        let prepared = prepare_import(raw).unwrap();
        assert!(prepared.contains_key(Path::new(MANIFEST)));
        assert!(validate_files(&prepared).is_ok());
        let bytes = encode(&prepared).unwrap();
        assert_eq!(decode(&bytes).unwrap(), prepared);
    }

    #[test]
    fn archive_rejects_incompressible_output_over_package_limit() {
        let mut bytes = vec![0_u8; MAX_BYTES as usize - 1024];
        let mut state = 0x9e37_79b9_u32;
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        let files = Files::from([
            ("world.wl".into(), b"event start".to_vec()),
            ("opaque.bin".into(), bytes),
        ]);
        assert!(validate_files(&files).is_ok());
        let error = prepare_import(files).unwrap_err();
        assert!(error.contains("压缩后的工程包超过 64 MiB"));
    }

    #[test]
    fn project_package_reopens_unsaved_new_and_deleted_json_documents() {
        let root = std::env::temp_dir().join(format!(
            "worldedit-package-roundtrip-{}",
            std::process::id()
        ));
        let reopen =
            root.with_file_name(format!("worldedit-package-reopen-{}", std::process::id()));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("notes.json"), b"ordinary json").unwrap();
        std::fs::write(root.join("assets/reference.bin"), [0, 9, 255]).unwrap();
        let mut project = worldline_core::project::Project::new(&root);
        let manifest = root.join(".world/project.json");
        let map = root.join(".world/maps/new.json");
        project
            .create_authoring_document(
                &manifest,
                br#"{"schema_version":1,"entry":"world.wl","maps":{"new":".world/maps/new.json"}}"#
                    .to_vec(),
            )
            .unwrap();
        let map_bytes = br#"{"opaque":17,"unknown":{"kept":true}}"#.to_vec();
        project
            .create_authoring_document(&map, map_bytes.clone())
            .unwrap();

        let mut package: Files = worldline_core::workspace_snapshot::snapshot_files(&project)
            .unwrap()
            .into_iter()
            .collect();
        package.insert(MANIFEST.into(), br#"{"entry":"world.wl"}"#.to_vec());
        let restored = decode(&encode(&package).unwrap()).unwrap();
        assert_eq!(
            restored[Path::new(".world/project.json")],
            package[Path::new(".world/project.json")]
        );
        assert_eq!(restored[Path::new(".world/maps/new.json")], map_bytes);
        assert_eq!(restored[Path::new("notes.json")], b"ordinary json");
        assert_eq!(restored[Path::new("assets/reference.bin")], [0, 9, 255]);

        for (relative, bytes) in &restored {
            let path = reopen.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let reopened = worldline_core::project::Project::open(&reopen).unwrap();
        assert_eq!(
            reopened
                .authoring_document(&reopen.join(".world/maps/new.json"))
                .unwrap()
                .bytes(),
            map_bytes
        );
        assert_eq!(
            std::fs::read(reopen.join("notes.json")).unwrap(),
            b"ordinary json"
        );
        assert_eq!(
            std::fs::read(reopen.join("assets/reference.bin")).unwrap(),
            [0, 9, 255]
        );

        project.delete_authoring_document(&map).unwrap();
        let deleted: Files = worldline_core::workspace_snapshot::snapshot_files(&project)
            .unwrap()
            .into_iter()
            .collect();
        assert!(!deleted.contains_key(Path::new(".world/maps/new.json")));
    }

    #[test]
    fn shared_export_roundtrips_into_the_desktop_compiler() {
        let root = std::env::temp_dir().join(format!("worldedit-web-test-{}", std::process::id()));
        let mut project = worldline_core::project::Project::new(&root);
        let before = project.compile().analysis.fingerprint;
        let files = decode(&encode(&project.export_files().unwrap()).unwrap()).unwrap();
        assert!(files.contains_key(Path::new("world.wl")));
        let sources = files
            .iter()
            .filter(|(p, _)| p.extension().is_some_and(|e| e == "wl"))
            .map(|(p, bytes)| (root.join(p), String::from_utf8(bytes.clone()).unwrap()))
            .collect();
        let result = worldline_core::compile_sources(&root.join(entry(&files).unwrap()), &sources);
        assert!(!result.has_errors());
        assert_eq!(before, result.analysis.fingerprint);
        project.mark_saved();
        assert!(!project.is_dirty());
        project
            .set_text(&project.entry.clone(), "broken draft".into())
            .unwrap();
        assert!(project.is_dirty());
    }
}
