//! 单次浏览器 SVG 选择；Drop 清理 DOM/回调，晚到读取只能写孤立状态。
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};

pub(super) struct SvgPicker {
    input: web_sys::HtmlInputElement,
    result: Rc<RefCell<Option<Result<String, String>>>>,
    _change: Closure<dyn FnMut(web_sys::Event)>,
    _cancel: Closure<dyn FnMut(web_sys::Event)>,
}

impl SvgPicker {
    pub(super) fn start(ctx: &egui::Context) -> Result<Self, String> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or("浏览器文档不可用")?;
        let input: web_sys::HtmlInputElement = document
            .create_element("input")
            .map_err(js_error)?
            .dyn_into()
            .map_err(|error: web_sys::Element| js_error(error.into()))?;
        input.set_type("file");
        input.set_accept(".svg,image/svg+xml");
        input.set_multiple(false);
        input
            .set_attribute("style", "display:none")
            .map_err(js_error)?;
        document
            .body()
            .ok_or("浏览器页面不可用")?
            .append_child(&input)
            .map_err(js_error)?;
        let result = Rc::new(RefCell::new(None));
        let selected = input.clone();
        let output = result.clone();
        let ctx_changed = ctx.clone();
        let change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let file = selected.files().and_then(|files| files.get(0));
            let output = output.clone();
            let ctx = ctx_changed.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let file = file.ok_or_else(|| "未选择 SVG".to_owned())?;
                    if file.size() > (2 * 1024 * 1024) as f64 {
                        return Err("SVG 超过 2 MiB 上限".into());
                    }
                    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                        .await
                        .map_err(js_error)?;
                    let array = js_sys::Uint8Array::new(&buffer);
                    if array.length() as usize > 2 * 1024 * 1024 {
                        return Err("SVG 超过 2 MiB 上限".into());
                    }
                    String::from_utf8(array.to_vec()).map_err(|_| "SVG 不是有效 UTF-8".into())
                }
                .await;
                *output.borrow_mut() = Some(result);
                ctx.request_repaint();
            });
        });
        let cancelled = result.clone();
        let ctx_cancelled = ctx.clone();
        let cancel = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            *cancelled.borrow_mut() = Some(Err("已取消 SVG 文件选择".into()));
            ctx_cancelled.request_repaint();
        });
        input.set_onchange(Some(change.as_ref().unchecked_ref()));
        input.set_oncancel(Some(cancel.as_ref().unchecked_ref()));
        input.click();
        Ok(Self {
            input,
            result,
            _change: change,
            _cancel: cancel,
        })
    }

    pub(super) fn poll(&mut self) -> Option<Result<String, String>> {
        self.result.borrow_mut().take()
    }
}

fn js_error(error: wasm_bindgen::JsValue) -> String {
    format!("SVG 文件选择失败：{error:?}")
}

impl Drop for SvgPicker {
    fn drop(&mut self) {
        self.input.set_onchange(None);
        self.input.set_oncancel(None);
        self.input.remove();
    }
}
