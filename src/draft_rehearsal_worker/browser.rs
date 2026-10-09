use super::protocol::*;
use js_sys::{Array, Object, Reflect, Uint8Array};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use worldline_core::{draft_rehearsal::DraftRehearsalRequest, project::Project};

pub(crate) struct SessionWorker {
    worker: web_sys::Worker,
    events: Rc<RefCell<VecDeque<Result<Response, String>>>>,
    on_message: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    on_error: Option<Closure<dyn FnMut(web_sys::ErrorEvent)>>,
    session_id: String,
    next: u64,
    expected: String,
    busy: bool,
}

impl SessionWorker {
    pub(crate) fn available() -> bool {
        true
    }
    #[cfg(test)]
    pub(crate) fn start(
        project: &Project,
        input: &DraftRehearsalRequest,
        session_id: String,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        Self::start_with_presentation(project, input, session_id, ctx, None)
    }

    pub(crate) fn start_with_presentation(
        project: &Project,
        input: &DraftRehearsalRequest,
        session_id: String,
        ctx: &egui::Context,
        presentation: Option<worldline_core::localization::LocalizationPresentationRequest>,
    ) -> Result<Self, String> {
        if presentation.is_some() {
            project
                .check_localization_budget()
                .map_err(|error| error.to_string())?;
        }
        let files = project.snapshot_files_limited(10_000, 128 * 1024 * 1024)?;
        let mut prepare = Prepare {
            schema_version: VERSION,
            session_id: session_id.clone(),
            request_id: "0".into(),
            input: input.clone(),
            entry: project
                .entry
                .strip_prefix(&project.root)
                .map_err(|_| "入口越界")?
                .into(),
            snapshot_state: project.snapshot_state()?,
            presentation,
        };
        let retained: Vec<_> = prepare
            .snapshot_state
            .documents
            .iter_mut()
            .filter_map(|document| {
                document
                    .retained_bytes
                    .take()
                    .map(|bytes| (document.path.clone(), bytes))
            })
            .collect();
        let json = serde_json::to_string(&prepare).map_err(|error| error.to_string())?;
        if json.len() > MAX_REQUEST {
            return Err("试演后台请求超过32MiB".into());
        }
        let window = web_sys::window().ok_or("没有浏览器Window")?;
        let document = window.document().ok_or("没有浏览器Document")?;
        let base = document
            .base_uri()
            .map_err(error)?
            .ok_or("页面基础URL不可用")?;
        let origin = window.location().origin().map_err(error)?;
        let module = asset(
            &document,
            "link[rel='modulepreload'][href$='.js']",
            &base,
            &origin,
        )?;
        let wasm = asset(
            &document,
            "link[rel='preload'][href$='.wasm']",
            &base,
            &origin,
        )?;
        let url = same_origin("rehearsal-worker.mjs", &base, &origin)?;
        let options = web_sys::WorkerOptions::new();
        options.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options(&url, &options).map_err(error)?;
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let queue = events.clone();
        let context = ctx.clone();
        let on_message = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            let value = event.data();
            let decoded = (|| {
                if let Ok(message) = string(&value, "error") {
                    return Err(message);
                }
                decode::<Response>(&string(&value, "response_json")?, MAX_RESPONSE)
            })();
            let mut queue = queue.borrow_mut();
            if queue.len() >= 2 {
                queue.clear();
                queue.push_back(Err("试演后台返回队列超额".into()));
            } else {
                queue.push_back(decoded);
            }
            context.request_repaint();
        }) as Box<dyn FnMut(_)>);
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let queue = events.clone();
        let context = ctx.clone();
        let on_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
            let mut queue = queue.borrow_mut();
            queue.clear();
            queue.push_back(Err(format!("试演后台意外中断：{}", event.message())));
            context.request_repaint();
        }) as Box<dyn FnMut(_)>);
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        let host = Self {
            worker,
            events,
            on_message: Some(on_message),
            on_error: Some(on_error),
            session_id,
            next: 1,
            expected: "0".into(),
            busy: true,
        };
        let payload = Object::new();
        set(&payload, "prepare_json", &JsValue::from_str(&json))?;
        set(&payload, "module_url", &JsValue::from_str(&module))?;
        set(&payload, "wasm_url", &JsValue::from_str(&wasm))?;
        let transfer = Array::new();
        set(&payload, "files", &pack(files.iter(), &transfer)?.into())?;
        set(
            &payload,
            "retained",
            &pack(retained.iter().map(|(p, b)| (p, b)), &transfer)?.into(),
        )?;
        host.worker
            .post_message_with_transfer(&payload, &transfer)
            .map_err(error)?;
        Ok(host)
    }

    pub(crate) fn busy(&self) -> bool {
        self.busy
    }
    pub(crate) fn submit(&mut self, action: Action) -> Result<(), String> {
        if self.busy {
            return Err("上一条试演请求尚未返回".into());
        }
        let command = Command {
            schema_version: VERSION,
            session_id: self.session_id.clone(),
            request_id: self.next.to_string(),
            action,
        };
        command.validate()?;
        let json = serde_json::to_string(&command).map_err(|e| e.to_string())?;
        if json.len() > MAX_REQUEST {
            return Err("试演命令超过预算".into());
        }
        let payload = Object::new();
        set(&payload, "command_json", &JsValue::from_str(&json))?;
        self.worker.post_message(&payload).map_err(error)?;
        self.expected = self.next.to_string();
        self.next = self.next.checked_add(1).ok_or("试演请求代次耗尽")?;
        self.busy = true;
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Option<Result<Response, String>> {
        let response = self.events.borrow_mut().pop_front()?;
        self.busy = false;
        let response = response.and_then(|response| {
            if response.schema_version != VERSION
                || response.session_id != self.session_id
                || response.request_id != self.expected
            {
                return Err("已拒绝不同会话或过期的试演结果".into());
            }
            Ok(response)
        });
        if response.is_err() {
            self.worker.terminate();
        }
        Some(response)
    }
}

impl Drop for SessionWorker {
    fn drop(&mut self) {
        self.worker.terminate();
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.on_message.take();
        self.on_error.take();
        self.events.borrow_mut().clear();
    }
}

fn asset(
    document: &web_sys::Document,
    selector: &str,
    base: &str,
    origin: &str,
) -> Result<String, String> {
    let link = document
        .query_selector(selector)
        .map_err(error)?
        .ok_or("缺少随编辑器分发的后台JS/WASM资源")?;
    same_origin(
        &link.get_attribute("href").ok_or("资源缺少href")?,
        base,
        origin,
    )
}
fn same_origin(value: &str, base: &str, origin: &str) -> Result<String, String> {
    let url = web_sys::Url::new_with_base(value, base).map_err(error)?;
    if url.origin() != origin || !matches!(url.protocol().as_str(), "http:" | "https:") {
        return Err("试演后台只能加载编辑器同源HTTP(S)静态资源".into());
    }
    Ok(url.href())
}
fn pack<'a>(
    files: impl Iterator<Item = (&'a std::path::PathBuf, &'a Vec<u8>)>,
    transfer: &Array,
) -> Result<Array, String> {
    let array = Array::new();
    let mut count = 0usize;
    let mut total = 0usize;
    for (path, bytes) in files {
        count += 1;
        total = total.saturating_add(bytes.len());
        if count > 10_000 || total > 128 * 1024 * 1024 {
            return Err("后台快照超过预算".into());
        }
        let item = Object::new();
        set(
            &item,
            "path",
            &JsValue::from_str(&crate::reader_zip::safe_snapshot_name(path)?),
        )?;
        let data = Uint8Array::from(bytes.as_slice());
        transfer.push(&data.buffer());
        set(&item, "bytes", &data)?;
        array.push(&item);
    }
    Ok(array)
}
fn set(object: &Object, key: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(error)
}
fn string(value: &JsValue, key: &str) -> Result<String, String> {
    Reflect::get(value, &JsValue::from_str(key))
        .map_err(error)?
        .as_string()
        .ok_or_else(|| format!("缺少{key}"))
}
fn error(error: JsValue) -> String {
    format!("浏览器试演失败：{error:?}")
}
