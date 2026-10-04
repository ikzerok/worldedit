//! 同源本地 Web Worker；UI 只收取 typed 结果，不信任任意候选文件。
use crate::archive::Files;
use crate::worker_protocol::*;
use js_sys::{Array, Object, Reflect, Uint8Array};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

pub(crate) struct WorkerJob {
    worker: web_sys::Worker,
    events: Rc<RefCell<VecDeque<WorkEvent>>>,
    on_message: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    on_error: Option<Closure<dyn FnMut(web_sys::ErrorEvent)>>,
}

impl WorkerJob {
    pub(crate) fn start(
        mut request: WorkRequest,
        files: &Files,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        request.validate()?;
        let retained = detach_retained(&mut request);
        let mut bytes = 0usize;
        let mut paths = std::collections::BTreeSet::new();
        if files.len().saturating_add(retained.len()) > MAX_FILES {
            return Err("后台快照文件数超过预算".into());
        }
        for (path, content) in files.iter().chain(retained.iter().map(|(p, b)| (p, b))) {
            crate::reader_zip::safe_snapshot_name(path)?;
            if !paths.insert(path) {
                return Err("后台快照路径重复或活动/墓碑重叠".into());
            }
            bytes = bytes
                .checked_add(content.len())
                .ok_or("后台快照字节数溢出")?;
            if bytes > MAX_BYTES {
                return Err("后台快照超过128MiB预算".into());
            }
        }
        let json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        if json.len() > MAX_JSON_BYTES {
            return Err("后台请求JSON超过32MiB预算".into());
        }
        let window = web_sys::window().ok_or("缺少浏览器Window")?;
        let document = window.document().ok_or("缺少浏览器Document")?;
        let base = document
            .base_uri()
            .map_err(js_error)?
            .ok_or("页面缺少基础URL")?;
        let origin = window.location().origin().map_err(js_error)?;
        let module = asset_url(
            &document,
            "link[rel='modulepreload'][href$='.js']",
            &base,
            &origin,
        )?;
        let wasm = asset_url(
            &document,
            "link[rel='preload'][href$='.wasm']",
            &base,
            &origin,
        )?;
        let worker_url = same_origin("worker.mjs", &base, &origin)?;
        let options = web_sys::WorkerOptions::new();
        options.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options(&worker_url, &options).map_err(js_error)?;
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let queue = events.clone();
        let context = ctx.clone();
        let expected = request.clone();
        let finish_worker = worker.clone();
        let on_message = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            let value = event.data();
            let decoded = receive(&value, &expected);
            let event = match decoded {
                Ok(Some(event)) => event,
                Ok(None) => return,
                Err(error) => WorkEvent::Error(error),
            };
            if push_event(&mut queue.borrow_mut(), event) {
                finish_worker.terminate();
            }
            context.request_repaint();
        }) as Box<dyn FnMut(_)>);
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let queue = events.clone();
        let context = ctx.clone();
        let finish_worker = worker.clone();
        let on_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
            if push_event(
                &mut queue.borrow_mut(),
                WorkEvent::Error(format!("后台任务意外中断：{}", event.message())),
            ) {
                finish_worker.terminate();
            }
            context.request_repaint();
        }) as Box<dyn FnMut(_)>);
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        let mut job = Self {
            worker,
            events,
            on_message: Some(on_message),
            on_error: Some(on_error),
        };
        let payload = Object::new();
        set(&payload, "request_json", &JsValue::from_str(&json))?;
        set(&payload, "module_url", &JsValue::from_str(&module))?;
        set(&payload, "wasm_url", &JsValue::from_str(&wasm))?;
        let transfer = Array::new();
        set(
            &payload,
            "files",
            &JsValue::from(pack(files.iter(), &transfer)?),
        )?;
        set(
            &payload,
            "retained",
            &JsValue::from(pack(retained.iter().map(|(p, b)| (p, b)), &transfer)?),
        )?;
        if let Err(error) = job.worker.post_message_with_transfer(&payload, &transfer) {
            job.cancel();
            return Err(js_error(error));
        }
        Ok(job)
    }

    pub(crate) fn take_event(&mut self) -> Option<WorkEvent> {
        let event = self.events.borrow_mut().pop_front();
        if matches!(event, Some(WorkEvent::Done { .. } | WorkEvent::Error(_))) {
            self.stop();
        }
        event
    }

    fn stop(&mut self) {
        self.worker.terminate();
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.on_message.take();
        self.on_error.take();
    }

    pub(crate) fn cancel(&mut self) {
        self.stop();
        self.events.borrow_mut().clear();
    }
}

impl Drop for WorkerJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn asset_url(
    document: &web_sys::Document,
    selector: &str,
    base: &str,
    origin: &str,
) -> Result<String, String> {
    let element = document
        .query_selector(selector)
        .map_err(js_error)?
        .ok_or("缺少随包分发的后台JS/WASM资源")?;
    let href = element.get_attribute("href").ok_or("后台资源缺少href")?;
    same_origin(&href, base, origin)
}

fn same_origin(value: &str, base: &str, origin: &str) -> Result<String, String> {
    let url = web_sys::Url::new_with_base(value, base).map_err(js_error)?;
    if url.origin() != origin || !matches!(url.protocol().as_str(), "http:" | "https:") {
        return Err("后台资源必须来自编辑器同源HTTP(S)静态包".into());
    }
    Ok(url.href())
}

fn pack<'a>(
    files: impl Iterator<Item = (&'a std::path::PathBuf, &'a Vec<u8>)>,
    transfer: &Array,
) -> Result<Array, String> {
    let result = Array::new();
    for (path, bytes) in files {
        let item = Object::new();
        set(
            &item,
            "path",
            &JsValue::from_str(&crate::reader_zip::safe_snapshot_name(path)?),
        )?;
        let data = Uint8Array::from(bytes.as_slice());
        transfer.push(&data.buffer());
        set(&item, "bytes", &data)?;
        result.push(&item);
    }
    Ok(result)
}

fn receive(value: &JsValue, request: &WorkRequest) -> Result<Option<WorkEvent>, String> {
    if string(value, "job_id")? != request.job_id
        || string(value, "baseline")? != request.baseline
        || string(value, "generation")? != request.generation.to_string()
    {
        return Ok(None);
    }
    match string(value, "event")?.as_str() {
        "progress" => Ok(Some(WorkEvent::Progress {
            stage: string(value, "stage")?,
            completed: count(value, "completed")?,
            total: count(value, "total")?,
        })),
        "error" => Ok(Some(WorkEvent::Error(string(value, "message")?))),
        "done" => {
            let json = string(value, "output_json")?;
            if json.len() > MAX_JSON_BYTES {
                return Err("后台返回JSON超额".into());
            }
            let output = parse_output_json(&json)?;
            let array = Reflect::get(value, &"binaries".into()).map_err(js_error)?;
            if !Array::is_array(&array) {
                return Err("后台二进制负载不是数组".into());
            }
            let array = Array::from(&array);
            if array.length() as usize > MAX_FILES + 1 {
                return Err("后台二进制项数超额".into());
            }
            let mut arrays = Vec::new();
            let mut total = 0usize;
            for item in array.iter() {
                let data = item
                    .dyn_into::<Uint8Array>()
                    .map_err(|_| "后台负载不是Uint8Array")?;
                total = total
                    .checked_add(data.length() as usize)
                    .ok_or("后台负载字节溢出")?;
                if total > MAX_BYTES * 2 {
                    return Err("后台文件和ZIP总负载超额".into());
                }
                arrays.push(data);
            }
            let lengths: Vec<_> = arrays.iter().map(|data| data.length() as usize).collect();
            request.accepts_lengths(&output, &lengths)?;
            let binaries = arrays.into_iter().map(|data| data.to_vec()).collect();
            Ok(Some(WorkEvent::Done {
                output: Box::new(output),
                binaries,
            }))
        }
        _ => Err("未知后台消息类型".into()),
    }
}

fn count(value: &JsValue, key: &str) -> Result<usize, String> {
    let number = Reflect::get(value, &key.into())
        .map_err(js_error)?
        .as_f64()
        .ok_or("后台计数无效")?;
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > usize::MAX as f64 {
        return Err("后台计数越界".into());
    }
    Ok(number as usize)
}
fn string(value: &JsValue, key: &str) -> Result<String, String> {
    Reflect::get(value, &key.into())
        .map_err(js_error)?
        .as_string()
        .ok_or_else(|| format!("后台消息缺少字符串{key}"))
}
fn set(object: &Object, key: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(object, &key.into(), value)
        .map(|_| ())
        .map_err(js_error)
}
fn js_error(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
