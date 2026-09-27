use super::paths::{register_path, reject_transactions, relative_path, ArchiveEntryKind};
#[cfg(any(target_arch = "wasm32", test))]
use super::{ensure_legacy_manifest, entry};
use super::{validate_files, Files, MAX_BYTES, MAX_FILES};
use std::io::{Cursor, Read, Write};
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
