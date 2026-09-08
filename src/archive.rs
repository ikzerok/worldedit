//! 浏览器工程包：保留目录与二进制素材，限制展开大小，拒绝越界路径。
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

pub type Files = BTreeMap<PathBuf, Vec<u8>>;
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 4096;
pub const MANIFEST: &str = "worldedit-project.json";

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
    if files.len() > MAX_FILES || files.values().map(|v| v.len() as u64).sum::<u64>() > MAX_BYTES {
        return Err("工程包最多包含 4096 个文件、64 MiB 内容".into());
    }
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, bytes) in files {
        let name = path.to_string_lossy().replace('\\', "/");
        relative_path(&name)?;
        archive
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        archive.write_all(bytes).map_err(|e| e.to_string())?;
    }
    archive
        .finish()
        .map(|out| out.into_inner())
        .map_err(|e| e.to_string())
}

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
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let path = relative_path(file.name())?;
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
    Ok(files)
}

pub fn entry(files: &Files) -> Result<PathBuf, String> {
    if let Some(manifest) = files.get(Path::new(MANIFEST)) {
        let value: serde_json::Value =
            serde_json::from_slice(manifest).map_err(|e| format!("工程入口记录无效：{e}"))?;
        let path = relative_path(value["entry"].as_str().ok_or("工程包缺少入口记录")?)?;
        if path.extension().is_some_and(|ext| ext == "wl") && files.contains_key(&path) {
            return Ok(path);
        }
        return Err("工程包记录的 .wl 入口文件不存在".into());
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
