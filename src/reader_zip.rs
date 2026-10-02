//! Reader 独立预算、可取消编码及逐字节读回；不改变完整工程备份限制。
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
#[cfg(any(target_arch = "wasm32", test))]
use std::path::Component;
use std::path::{Path, PathBuf};

pub(crate) const MAX_FILES: usize = 10_000;
pub(crate) const MAX_BYTES: usize = 128 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;
const CANCELLED: &str = "READER_CANCELLED：已取消阅读包生成";

/// 回调计数单位为文件（编码与核对各一次），每次底层写入及64KiB读取都检查取消。
pub(crate) fn encode(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    control: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<Vec<u8>, String> {
    encode_with_budget(files, control, MAX_BYTES)
}

fn encode_with_budget(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    control: &mut dyn FnMut(usize, usize) -> bool,
    zip_budget: usize,
) -> Result<Vec<u8>, String> {
    validate(files)?;
    let failure = Cell::new(None);
    let total = files.len() * 2;
    let control = RefCell::new(control);
    if !(control.borrow_mut())(0, total) {
        return Err(CANCELLED.into());
    }
    let completed = Cell::new(0);
    let destination = ControlledCursor {
        cursor: Cursor::new(Vec::new()),
        control: &control,
        completed: &completed,
        total,
        position: 0,
        length: 0,
        budget: zip_budget as u64,
        failure: &failure,
    };
    let mut writer = zip::ZipWriter::new(destination);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (index, (path, bytes)) in files.iter().enumerate() {
        completed.set(index);
        writer
            .start_file(safe_name(path)?, options)
            .map_err(|error| format!("阅读包ZIP条目创建失败：{error}"))?;
        check_failure(&failure)?;
        for chunk in bytes.chunks(CHUNK) {
            if !(control.borrow_mut())(index, total) {
                failure.set(Some(AbortReason::Cancelled));
                return Err(CANCELLED.into());
            }
            writer
                .write_all(chunk)
                .map_err(|error| format!("阅读包ZIP写入失败：{error}"))?;
            check_failure(&failure)?;
        }
    }
    let destination = writer
        .finish()
        .map_err(|error| format!("阅读包ZIP结束失败：{error}"))?;
    check_failure(&failure)?;
    let bytes = destination.cursor.into_inner();
    let control = destination.control;
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes))
        .map_err(|error| format!("阅读包ZIP核对失败：{error}"))?;
    if archive.len() != files.len() {
        return Err("阅读包ZIP文件数与审核集合不一致".into());
    }
    let mut buffer = [0u8; CHUNK];
    for (index, (path, expected)) in files.iter().enumerate() {
        if !(control.borrow_mut())(files.len() + index, total) {
            return Err(CANCELLED.into());
        }
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("阅读包ZIP读取失败：{error}"))?;
        if entry.name() != safe_name(path)? || entry.size() != expected.len() as u64 {
            return Err("阅读包ZIP身份或大小与审核集合不一致".into());
        }
        let mut offset = 0;
        loop {
            if !(control.borrow_mut())(files.len() + index, total) {
                return Err(CANCELLED.into());
            }
            let length = entry
                .read(&mut buffer)
                .map_err(|error| format!("阅读包ZIP读回失败：{error}"))?;
            if length == 0 {
                break;
            }
            if expected.get(offset..offset + length) != Some(&buffer[..length]) {
                return Err("阅读包ZIP逐字节核对失败".into());
            }
            offset += length;
        }
        if offset != expected.len() {
            return Err("阅读包ZIP读回长度不足".into());
        }
    }
    if !(control.borrow_mut())(total, total) {
        return Err(CANCELLED.into());
    }
    Ok(bytes)
}

pub(crate) fn validate(files: &BTreeMap<PathBuf, Vec<u8>>) -> Result<usize, String> {
    if files.len() > MAX_FILES {
        return Err("阅读包超过10000文件预算".into());
    }
    let mut total = 0usize;
    for (path, bytes) in files {
        safe_name(path)?;
        if path
            .ancestors()
            .skip(1)
            .any(|parent| files.contains_key(parent))
        {
            return Err("阅读包文件与目录路径冲突".into());
        }
        total = total.checked_add(bytes.len()).ok_or("阅读包大小溢出")?;
        if total > MAX_BYTES {
            return Err("阅读包超过128MiB原始内容预算".into());
        }
    }
    Ok(total)
}

pub(crate) fn safe_name(path: &Path) -> Result<String, String> {
    worldline_core::reader_export::portable_output_path(path)
}

/// 完整工作区快照不是公开URL：合法的#/%/&等原始文件名必须保留。
/// Windows的原生相对路径仅正规化分隔符，其他平台不接受字面反斜杠。
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn safe_snapshot_name(path: &Path) -> Result<String, String> {
    let raw = path.to_str().ok_or("工作区快照路径必须为UTF-8")?;
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("工作区快照路径不安全：{raw}"));
    }
    snapshot_name_text(raw, cfg!(windows))
}

#[cfg(any(target_arch = "wasm32", test))]
fn snapshot_name_text(raw: &str, windows_separators: bool) -> Result<String, String> {
    if raw.is_empty() || raw.contains([':', '\0']) || (!windows_separators && raw.contains('\\')) {
        return Err(format!("工作区快照路径不安全：{raw}"));
    }
    let parts: Vec<_> = raw
        .split(|character| character == '/' || (windows_separators && character == '\\'))
        .collect();
    if parts.iter().any(|part| matches!(*part, "" | "." | "..")) {
        return Err(format!("工作区快照路径不安全：{raw}"));
    }
    Ok(parts.join("/"))
}

#[derive(Clone, Copy)]
enum AbortReason {
    Cancelled,
    Budget,
}

fn check_failure(failure: &Cell<Option<AbortReason>>) -> Result<(), String> {
    match failure.get() {
        None => Ok(()),
        Some(AbortReason::Cancelled) => Err(CANCELLED.into()),
        Some(AbortReason::Budget) => Err("阅读包超过128MiB ZIP预算".into()),
    }
}

/// zip 2.x 的头部回写不能被可恢复的 Write 错误打断后再 Drop。
/// 锁存失败后只维护虚拟位置、丢弃后续字节；外层绝不会返回这个废弃包。
/// 这样中止不会再次触发库的 finalize 位置断言，也不分配超预算字节。
struct ControlledCursor<'a, 'callback> {
    cursor: Cursor<Vec<u8>>,
    control: &'a RefCell<&'callback mut dyn FnMut(usize, usize) -> bool>,
    completed: &'a Cell<usize>,
    total: usize,
    position: u64,
    length: u64,
    budget: u64,
    failure: &'a Cell<Option<AbortReason>>,
}

impl ControlledCursor<'_, '_> {
    fn observe_cancel(&self) {
        if self.failure.get().is_none()
            && !(self.control.borrow_mut())(self.completed.get(), self.total)
        {
            self.failure.set(Some(AbortReason::Cancelled));
        }
    }
}

impl Write for ControlledCursor<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.observe_cancel();
        let end = self.position.saturating_add(bytes.len() as u64);
        if end > self.budget && self.failure.get().is_none() {
            self.failure.set(Some(AbortReason::Budget));
        }
        if self.failure.get().is_none() {
            self.cursor.set_position(self.position);
            self.cursor.write_all(bytes)?;
        }
        self.position = end;
        self.length = self.length.max(end);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.observe_cancel();
        Ok(())
    }
}

impl Seek for ControlledCursor<'_, '_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let target = match position {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
            SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
        };
        let target = u64::try_from(target)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "无效ZIP位置"))?;
        if target > self.budget && self.failure.get().is_none() {
            self.failure.set(Some(AbortReason::Budget));
        }
        self.position = target;
        Ok(target)
    }
}

#[cfg(test)]
mod tests;
