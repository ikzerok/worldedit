//! 浏览器宿主：文件授权、下载和本地保存，不承载语言解析或界面业务。
pub use crate::archive::Files;
use crate::archive::{self, MAX_BYTES};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use worldline_core::{catalog::TargetRef, catalog_edit::AssetDraft};

#[derive(Clone)]
pub enum FileAction {
    Open,
    Include,
    Attach(TargetRef),
    Replace(AssetDraft),
}
pub type FileEvent = (FileAction, Result<Files, String>);

thread_local! {
    static EVENTS: RefCell<VecDeque<FileEvent>> = const { RefCell::new(VecDeque::new()) };
    static IMPORTED: RefCell<Files> = const { RefCell::new(Files::new()) };
    static DIRTY: Cell<bool> = const { Cell::new(false) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
    static SAVED_BASELINE: RefCell<Option<String>> = const { RefCell::new(None) };
    static PICKER: RefCell<Option<Picker>> = const { RefCell::new(None) };
}
struct Picker {
    input: web_sys::HtmlInputElement,
    _change: Closure<dyn FnMut(web_sys::Event)>,
    _cancel: Closure<dyn FnMut(web_sys::Event)>,
}
impl Drop for Picker {
    fn drop(&mut self) {
        self.input.set_onchange(None);
        self.input.set_oncancel(None);
        self.input.remove();
    }
}

fn error(value: JsValue) -> String {
    format!("浏览器操作失败：{value:?}")
}

pub fn start() {
    spawn_local(async {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id("worldedit-canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let mut app = crate::app::WorldeditApp::new(cc, None);
                    app.restore_browser_save();
                    Ok(Box::new(app))
                }),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_text_content(Some(&format!(
                    "无法启动 worldedit：{e:?}。请使用支持 WebGL 的浏览器并启用硬件加速。"
                ))),
            }
        }
    });
    let unload = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(
        |event: web_sys::BeforeUnloadEvent| {
            if DIRTY.with(Cell::get) {
                event.prevent_default();
                event.set_return_value("有未保存修改");
            }
        },
    );
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("beforeunload", unload.as_ref().unchecked_ref())
        .unwrap();
    unload.forget(); // 与页面同寿命。
}

pub fn set_dirty(value: bool) {
    DIRTY.set(value);
}

pub fn toggle_fullscreen() {
    let document = web_sys::window().unwrap().document().unwrap();
    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
    } else if let Some(element) = document.document_element() {
        let _ = element.request_fullscreen();
    }
}
pub fn take_event() -> Option<FileEvent> {
    EVENTS.with(|events| events.borrow_mut().pop_front())
}
pub fn imported() -> Files {
    IMPORTED.with(|files| files.borrow().clone())
}

pub fn imported_size(path: &Path) -> Option<usize> {
    let relative = path.strip_prefix(Path::new("/world")).ok()?;
    IMPORTED.with(|files| files.borrow().get(relative).map(Vec::len))
}

pub fn mount(files: Files) {
    GENERATION.set(GENERATION.get().wrapping_add(1));
    worldline_core::file_access::mount(
        files
            .iter()
            .map(|(p, b)| (Path::new("/world").join(p), b.clone()))
            .collect(),
    );
    IMPORTED.with(|current| *current.borrow_mut() = files);
}

/// 文件选择发生在点击事件里，保持浏览器用户授权；读取完成后交回共享 GUI。
pub fn select_files(ctx: &egui::Context, folder: bool, accept: &str, action: FileAction) {
    let result = (|| -> Result<(), JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
        input.set_type("file");
        input.set_multiple(!matches!(action, FileAction::Replace(_)));
        input.set_accept(accept);
        input.set_attribute("style", "display:none")?;
        if folder {
            input.set_attribute("webkitdirectory", "")?;
        }
        document.body().unwrap().append_child(&input)?;
        let selected = input.clone();
        let generation = GENERATION.get();
        let context = ctx.clone();
        let picked_action = action.clone();
        let change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let list = selected.files();
            let context = context.clone();
            let action = picked_action.clone();
            spawn_local(async move {
                let result = read_files(list, folder).await.and_then(|files| {
                    if generation == GENERATION.get() {
                        Ok(files)
                    } else {
                        Err("工程已切换，请重新选择文件".into())
                    }
                });
                EVENTS.with(|events| events.borrow_mut().push_back((action, result)));
                PICKER.with(|picker| picker.borrow_mut().take());
                context.request_repaint();
            });
        });
        let cancel = Closure::<dyn FnMut(web_sys::Event)>::new(|_| {
            spawn_local(async {
                PICKER.with(|picker| picker.borrow_mut().take());
            });
        });
        input.set_onchange(Some(change.as_ref().unchecked_ref()));
        input.set_oncancel(Some(cancel.as_ref().unchecked_ref()));
        PICKER.with(|picker| {
            *picker.borrow_mut() = Some(Picker {
                input: input.clone(),
                _change: change,
                _cancel: cancel,
            })
        });
        input.click();
        Ok(())
    })();
    if let Err(e) = result {
        EVENTS.with(|events| events.borrow_mut().push_back((action, Err(error(e)))));
    }
}

async fn read_files(list: Option<web_sys::FileList>, folder: bool) -> Result<Files, String> {
    let list = list.ok_or("未选择文件")?;
    if list.length() > 4096 {
        return Err("工程最多包含 4096 个文件".into());
    }
    let mut total = 0_u64;
    let mut files = Files::new();
    for index in 0..list.length() {
        let file = list.get(index).ok_or("选中的文件无法读取")?;
        total += file.size() as u64;
        if total > MAX_BYTES {
            return Err("所选文件总大小超过 64 MiB".into());
        }
        let name = if folder {
            let relative = js_sys::Reflect::get(&file, &JsValue::from_str("webkitRelativePath"))
                .map_err(error)?
                .as_string()
                .ok_or("浏览器未提供文件相对路径")?;
            relative
                .split_once('/')
                .ok_or("文件夹路径无效")?
                .1
                .to_owned()
        } else {
            file.name()
        };
        let path = archive::relative_path(&name)?;
        let buffer = JsFuture::from(file.array_buffer()).await.map_err(error)?;
        if files
            .insert(path, js_sys::Uint8Array::new(&buffer).to_vec())
            .is_some()
        {
            return Err("选择了同名文件，请改为选择整个工程文件夹".into());
        }
    }
    Ok(files)
}

pub fn add_files(files: Files, assets: bool) -> Result<Vec<PathBuf>, String> {
    let mut current = imported();
    let mut paths = Vec::new();
    for (relative, bytes) in files {
        let relative = if assets {
            Path::new("assets").join(relative)
        } else {
            relative
        };
        let mut destination = relative.clone();
        let mut suffix = 1;
        while current
            .get(&destination)
            .is_some_and(|existing| existing != &bytes)
        {
            if !assets {
                return Err(format!("文件已存在：{}", relative.display()));
            }
            destination = relative.with_file_name(format!(
                "{}-{suffix}{}",
                relative.file_stem().unwrap_or_default().to_string_lossy(),
                relative
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default()
            ));
            suffix += 1;
        }
        paths.push(Path::new("/world").join(&destination));
        current.insert(destination, bytes);
    }
    if current.values().map(|v| v.len() as u64).sum::<u64>() > MAX_BYTES {
        return Err("工程素材总大小超过 64 MiB".into());
    }
    mount(current);
    Ok(paths)
}

pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(error)?;
    let document = web_sys::window().unwrap().document().unwrap();
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(error)?
        .dyn_into()
        .map_err(|e: web_sys::Element| error(e.into()))?;
    anchor.set_href(&url);
    anchor.set_download(name);
    document
        .body()
        .unwrap()
        .append_child(&anchor)
        .map_err(error)?;
    anchor.click();
    anchor.remove();
    let release = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(release.unchecked_ref(), 30_000)
        .map_err(error)?;
    Ok(())
}

const STORAGE_KEY: &str = "worldedit.project.v1";
pub fn persist(bytes: &[u8]) -> Result<(), String> {
    let window = web_sys::window().unwrap();
    let binary: String = bytes.iter().map(|b| char::from(*b)).collect();
    let encoded = window.btoa(&binary).map_err(error)?;
    let storage = window
        .local_storage()
        .map_err(error)?
        .ok_or("浏览器不允许本地保存")?;
    let current = storage.get_item(STORAGE_KEY).map_err(error)?;
    if !SAVED_BASELINE.with(|baseline| *baseline.borrow() == current) {
        return Err(
            "另一标签页已更改浏览器存档，未覆盖。工程包已请求下载，请确认下载完成后刷新页面合并。"
                .into(),
        );
    }
    storage.set_item(STORAGE_KEY, &encoded).map_err(|_| {
        "浏览器本地存储空间不足。工程包已请求下载，请确认下载完成；未保存标记继续保留。".to_string()
    })?;
    SAVED_BASELINE.with(|baseline| *baseline.borrow_mut() = Some(encoded));
    Ok(())
}
pub fn restore() -> Result<Option<Files>, String> {
    let window = web_sys::window().unwrap();
    let Some(storage) = window.local_storage().map_err(error)? else {
        return Ok(None);
    };
    let Some(encoded) = storage.get_item(STORAGE_KEY).map_err(error)? else {
        return Ok(None);
    };
    SAVED_BASELINE.with(|baseline| *baseline.borrow_mut() = Some(encoded.clone()));
    let binary = window.atob(&encoded).map_err(error)?;
    let bytes: Vec<_> = binary.chars().map(|c| c as u8).collect();
    archive::decode(&bytes).map(Some)
}
