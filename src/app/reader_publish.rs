//! 显式选择并交付静态读者包；分析、过滤和 HTML 渲染都由 worldline-core 完成。

mod delivery;
mod preview;
mod selection;
mod window;

use crate::archive;
use std::collections::{BTreeMap, BTreeSet};
use worldline_core::catalog::TargetRef;
use worldline_core::reader_export::{ReaderExportPreview, ReaderExportSelection};

#[derive(Default)]
pub(super) struct ReaderPublishState {
    pub(super) open: bool,
    site_title: String,
    object_choices: Vec<ObjectChoice>,
    manuscript_choices: Vec<ManuscriptChoice>,
    attachment_choices: Vec<AttachmentChoice>,
    objects: BTreeSet<TargetRef>,
    chapters: BTreeMap<String, BTreeSet<String>>,
    attachments: BTreeSet<String>,
    #[cfg(not(target_arch = "wasm32"))]
    destination: String,
    reviewed: Option<ReviewedPackage>,
    confirmed: bool,
    status: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<ReaderPublishJob>,
}

struct ReviewedPackage {
    selection: ReaderExportSelection,
    preview: ReaderExportPreview,
    files: archive::Files,
    zip: Vec<u8>,
    public_pages: Vec<(String, String, String)>,
    raw_bytes: usize,
}

#[cfg(not(target_arch = "wasm32"))]
struct ReaderPublishJob {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: std::sync::mpsc::Receiver<ReaderPublishMessage>,
}

#[cfg(not(target_arch = "wasm32"))]
enum ReaderPublishMessage {
    Stage(&'static str),
    Done(Box<Result<ReviewedPackage, String>>),
}

#[derive(Clone, PartialEq, Eq)]
struct ManuscriptChoice {
    id: String,
    title: String,
    chapters: Vec<(String, String)>,
    unavailable: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct ObjectChoice {
    target: TargetRef,
    display: String,
}

#[derive(Clone, PartialEq, Eq)]
struct AttachmentChoice {
    id: String,
    display: String,
    available: bool,
}

enum PublishAction {
    Preview(ReaderExportSelection),
    Cancel,
    Publish,
    #[cfg(not(target_arch = "wasm32"))]
    Browse,
}
