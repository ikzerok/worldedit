#[cfg(any(target_arch = "wasm32", test))]
use super::{
    decode, entry, Files, MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64, MAX_BROWSER_RECOVERY_BYTES,
    MAX_BROWSER_SNAPSHOT_ENVELOPE_BYTES, MAX_BYTES, RECOVERY_CHECKPOINTS, RECOVERY_MANIFEST,
    RECOVERY_PROJECT,
};

#[cfg(any(target_arch = "wasm32", test))]
use std::io::{Cursor, Read, Write};
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
