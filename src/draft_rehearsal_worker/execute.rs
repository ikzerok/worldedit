use super::{engine::Engine, protocol::*};
use js_sys::{Array, Reflect, Uint8Array};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use wasm_bindgen::{prelude::*, JsCast};
use worldline_core::project::Project;
use worldline_runtime::ReplayCancellation;

thread_local! { static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) }; }

#[wasm_bindgen]
pub fn rehearsal_worker_prepare(
    json: &str,
    files: JsValue,
    retained: JsValue,
) -> Result<String, JsValue> {
    prepare(json, files, retained).map_err(|error| JsValue::from_str(&error))
}
fn prepare(json: &str, files: JsValue, retained: JsValue) -> Result<String, String> {
    if web_sys::window().is_some() {
        return Err("试演后台入口不能在编辑器Window执行".into());
    }
    if ENGINE.with(|engine| engine.borrow().is_some()) {
        return Err("Worker已持有一份试演".into());
    }
    let mut request: Prepare = decode(json, MAX_REQUEST)?;
    identity(
        request.schema_version,
        &request.session_id,
        &request.request_id,
    )?;
    crate::reader_zip::safe_snapshot_name(&request.entry)?;
    let mut total = 0usize;
    let files = read_files(files, &mut total)?;
    let mut retained = read_files(retained, &mut total)?;
    if files.len().saturating_add(retained.len()) > 10_000
        || files.keys().any(|path| retained.contains_key(path))
    {
        return Err("试演快照文件身份重叠或超过预算".into());
    }
    for document in &mut request.snapshot_state.documents {
        if document.retained_bytes.is_some() {
            return Err("墓碑字节必须经二进制侧通道传输".into());
        }
        if let Some(bytes) = retained.remove(&document.path) {
            if !document.deleted {
                return Err("非墓碑不能接收保留字节".into());
            }
            document.retained_bytes = Some(bytes);
        }
    }
    if !retained.is_empty() {
        return Err("存在未登记墓碑字节".into());
    }
    let root = Path::new("/world");
    worldline_core::file_access::mount(
        files
            .iter()
            .map(|(path, bytes)| (root.join(path), bytes.clone()))
            .collect(),
    );
    let project =
        Project::from_snapshot_with_state(root, &request.entry, &files, &request.snapshot_state)?;
    if project.content_baseline() != request.input.content_baseline {
        return Err("后台完整工程基线不匹配".into());
    }
    let engine = Engine::prepare(&project, &request)?;
    let response = encode(&engine.prepared())?;
    ENGINE.with(|current| *current.borrow_mut() = Some(engine));
    Ok(response)
}

#[wasm_bindgen]
pub fn rehearsal_worker_command(json: &str) -> Result<String, JsValue> {
    command(json).map_err(|error| JsValue::from_str(&error))
}
fn command(json: &str) -> Result<String, String> {
    if web_sys::window().is_some() {
        return Err("试演后台入口不能在Window执行".into());
    }
    let request: Command = decode(json, MAX_REQUEST)?;
    ENGINE.with(|engine| {
        let mut current = engine.borrow_mut();
        let response = current
            .as_mut()
            .ok_or("试演尚未准备")?
            .execute(request, &ReplayCancellation::new());
        match encode(&response) {
            Ok(json) => Ok(json),
            Err(error) => {
                *current = None;
                Err(error)
            }
        }
    })
}

fn read_files(value: JsValue, total: &mut usize) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    if !Array::is_array(&value) {
        return Err("后台快照负载不是数组".into());
    }
    let array = Array::from(&value);
    if array.length() > 10_000 {
        return Err("后台快照文件数超限".into());
    }
    let mut result = BTreeMap::new();
    for item in array.iter() {
        let path = Reflect::get(&item, &"path".into())
            .map_err(|_| "无法读取文件路径")?
            .as_string()
            .ok_or("文件路径不是字符串")?;
        let path = PathBuf::from(path);
        crate::reader_zip::safe_snapshot_name(&path)?;
        let bytes = Reflect::get(&item, &"bytes".into())
            .map_err(|_| "无法读取文件字节")?
            .dyn_into::<Uint8Array>()
            .map_err(|_| "文件字节不是Uint8Array")?;
        *total = total
            .checked_add(bytes.length() as usize)
            .ok_or("后台快照字节溢出")?;
        if *total > 128 * 1024 * 1024 {
            return Err("后台快照超过128MiB".into());
        }
        if result.insert(path, bytes.to_vec()).is_some() {
            return Err("后台快照路径重复".into());
        }
    }
    Ok(result)
}
