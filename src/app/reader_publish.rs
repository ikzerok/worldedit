//! 显式选择并交付静态读者包；分析、过滤和 HTML 渲染都由 worldline-core 完成。

use super::WorldeditApp;
use crate::archive;
use egui::{RichText, TextEdit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use worldline_core::catalog::TargetRef;
use worldline_core::manuscript::ManuscriptEntryKind;
use worldline_core::project::Project;
use worldline_core::reader_export::{
    ReaderExportPreview, ReaderExportSelection, ReaderManuscriptSelection,
};

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

impl ReaderPublishState {
    fn new() -> Self {
        Self {
            site_title: "离线阅读包".into(),
            #[cfg(not(target_arch = "wasm32"))]
            destination: "reader-site.zip".into(),
            ..Self::default()
        }
    }

    fn invalidate_review(&mut self) {
        self.reviewed = None;
        self.confirmed = false;
        self.status = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.job = None;
        }
    }

    fn selection(&self) -> ReaderExportSelection {
        ReaderExportSelection {
            schema_version: worldline_core::reader_export::READER_EXPORT_SCHEMA_VERSION,
            site_title: self.site_title.clone(),
            objects: self.objects.iter().cloned().collect(),
            manuscripts: self
                .chapters
                .iter()
                .filter(|(_, chapters)| !chapters.is_empty())
                .map(|(id, chapters)| ReaderManuscriptSelection {
                    id: id.clone(),
                    chapters: chapters.iter().cloned().collect(),
                })
                .collect(),
            attachments: self.attachments.iter().cloned().collect(),
        }
    }

    fn has_selection(&self) -> bool {
        !self.objects.is_empty()
            || self.chapters.values().any(|chapters| !chapters.is_empty())
            || !self.attachments.is_empty()
    }

    pub(super) fn refresh_choices(
        &mut self,
        project: &Project,
        snapshot: Option<&super::Snapshot>,
    ) {
        let object_choices = snapshot
            .map(|snapshot| {
                snapshot
                    .result
                    .analysis
                    .catalog
                    .objects
                    .iter()
                    .filter(|object| {
                        matches!(
                            object.target.kind.as_str(),
                            "event"
                                | "scene"
                                | "character"
                                | "entity"
                                | "world"
                                | "storyline"
                                | "period"
                                | "anchor"
                                | "state"
                                | "tag"
                                | "relation"
                                | "variable"
                        )
                    })
                    .map(|object| ObjectChoice {
                        target: object.target.clone(),
                        display: object.display.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let attachment_choices = snapshot
            .map(|snapshot| {
                snapshot
                    .result
                    .analysis
                    .catalog
                    .assets
                    .values()
                    .map(|asset| AttachmentChoice {
                        id: asset.id.clone(),
                        display: asset.display.clone(),
                        available: asset.available,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let manuscript_choices = project
            .manuscript_indices()
            .into_iter()
            .map(|(id, index)| {
                let chapters = index
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                    .map(|entry| (entry.id.clone(), entry.title.clone()))
                    .collect();
                ManuscriptChoice {
                    id: id.clone(),
                    title: index.title.unwrap_or(id),
                    chapters,
                    unavailable: (index.read_only || !index.diagnostics.is_empty()).then(|| {
                        if index.read_only {
                            "只读书稿，不能公开".into()
                        } else {
                            format!("书稿存在 {} 项结构诊断，不能公开", index.diagnostics.len())
                        }
                    }),
                }
            })
            .collect::<Vec<_>>();

        let allowed_objects: BTreeSet<_> = object_choices
            .iter()
            .map(|choice| choice.target.clone())
            .collect();
        let allowed_attachments: BTreeSet<_> = attachment_choices
            .iter()
            .map(|choice| choice.id.clone())
            .collect();
        self.objects
            .retain(|target| allowed_objects.contains(target));
        self.attachments
            .retain(|id| allowed_attachments.contains(id));
        self.chapters.retain(|book_id, chapters| {
            let Some(book) = manuscript_choices.iter().find(|book| book.id == *book_id) else {
                return false;
            };
            let allowed_chapters: BTreeSet<_> =
                book.chapters.iter().map(|(id, _)| id.as_str()).collect();
            chapters.retain(|id| allowed_chapters.contains(id.as_str()));
            !chapters.is_empty()
        });
        self.object_choices = object_choices;
        self.manuscript_choices = manuscript_choices;
        self.attachment_choices = attachment_choices;
        self.invalidate_review();
    }
}

impl WorldeditApp {
    pub(super) fn open_reader_publish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let state = {
            let mut state = ReaderPublishState::new();
            let stem = self
                .project
                .root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "world-project".into());
            let parent = self.project.root.parent().unwrap_or(&self.project.root);
            state.destination = parent
                .join(format!("{stem}-reader-site.zip"))
                .to_string_lossy()
                .into_owned();
            state
        };
        #[cfg(target_arch = "wasm32")]
        let state = ReaderPublishState::new();
        self.reader_publish = state;
        self.reader_publish
            .refresh_choices(&self.project, self.snapshot.as_ref());
        self.reader_publish.open = true;
    }

    pub(super) fn reader_publish_window(&mut self, ctx: &egui::Context) {
        self.poll_reader_publish_job();
        #[cfg(not(target_arch = "wasm32"))]
        if self.reader_publish.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.reader_publish.open {
            return;
        }

        let objects = self.reader_publish.object_choices.clone();
        let attachments = self.reader_publish.attachment_choices.clone();
        let manuscripts = self.reader_publish.manuscript_choices.clone();

        let mut action = None;
        let mut open = self.reader_publish.open;
        let state = &mut self.reader_publish;
        egui::Window::new("发布给读者")
            .id(egui::Id::new("reader-publish-window"))
            .open(&mut open)
            .resizable(true)
            .default_width(780.0)
            .default_height(980.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("只生成明确选择的离线静态内容。完整工程备份仍保留原有全部文件。").color(crate::theme::MUTED));
                ui.label(RichText::new("离线选择不是权限认证；拿到阅读包的人可以查看包内全部内容。").strong().color(crate::theme::GOLD));
                if let Some(status) = &state.status {
                    ui.label(status);
                }
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("读者站点标题");
                    changed |= ui
                        .add(TextEdit::singleline(&mut state.site_title).desired_width(360.0))
                        .changed();
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("reader-publish-choices")
                    .max_height(440.0)
                    .show(ui, |ui| {
                        ui.heading(format!("资料对象（{}）", objects.len()));
                        for choice in &objects {
                            let mut selected = state.objects.contains(&choice.target);
                            let label = format!(
                                "{} · {} ({})",
                                choice.display, choice.target.kind, choice.target.id
                            );
                            if ui.checkbox(&mut selected, label).changed() {
                                if selected {
                                    state.objects.insert(choice.target.clone());
                                } else {
                                    state.objects.remove(&choice.target);
                                }
                                changed = true;
                            }
                        }
                        if objects.is_empty() {
                            ui.label("当前工程没有可选择的资料对象。");
                        }

                        ui.add_space(8.0);
                        ui.heading("书稿章节");
                        for book in &manuscripts {
                            ui.label(RichText::new(&book.title).strong());
                            if let Some(reason) = &book.unavailable {
                                ui.label(RichText::new(reason).color(crate::theme::GOLD));
                                continue;
                            }
                            for (chapter_id, chapter_title) in &book.chapters {
                                let selected = state
                                    .chapters
                                    .get(&book.id)
                                    .is_some_and(|chapters| chapters.contains(chapter_id));
                                let mut selected = selected;
                                if ui
                                    .checkbox(
                                        &mut selected,
                                        format!("{} ({})", chapter_title, chapter_id),
                                    )
                                    .changed()
                                {
                                    let chapters = state.chapters.entry(book.id.clone()).or_default();
                                    if selected {
                                        chapters.insert(chapter_id.clone());
                                    } else {
                                        chapters.remove(chapter_id);
                                    }
                                    changed = true;
                                }
                            }
                            if book.chapters.is_empty() {
                                ui.label("没有可选择的章节。");
                            }
                        }
                        if manuscripts.is_empty() {
                            ui.label("当前工程没有注册书稿。");
                        }

                        ui.add_space(8.0);
                        ui.heading(format!("附件（{}）", attachments.len()));
                        for choice in &attachments {
                            let mut selected = state.attachments.contains(&choice.id);
                            let label = format!("{} ({})", choice.display, choice.id);
                            let response = ui.add_enabled(
                                choice.available,
                                egui::Checkbox::new(&mut selected, label),
                            );
                            if response.changed() {
                                if selected {
                                    state.attachments.insert(choice.id.clone());
                                } else {
                                    state.attachments.remove(&choice.id);
                                }
                                changed = true;
                            }
                            if !choice.available {
                                ui.label(RichText::new("此附件当前不可用。").color(crate::theme::GOLD));
                            }
                        }
                        if attachments.is_empty() {
                            ui.label("当前工程没有已登记的附件。");
                        }
                    });
                if changed {
                    state.invalidate_review();
                }

                ui.separator();
                #[cfg(not(target_arch = "wasm32"))]
                ui.horizontal(|ui| {
                    ui.label("ZIP 目标（新文件，工作区外）");
                    let response = ui.add(
                        TextEdit::singleline(&mut state.destination)
                            .desired_width(300.0)
                            .hint_text("选择一个尚不存在的 .zip 文件"),
                    );
                    if response.changed() {
                        state.confirmed = false;
                    }
                    if ui.button("浏览…").clicked() {
                        action = Some(PublishAction::Browse);
                    }
                });
                #[cfg(target_arch = "wasm32")]
                ui.label("确认后将下载 `worldedit-reader-site.zip`；失败时会显示错误，不改变工程保存状态。");

                if let Some(reviewed) = &state.reviewed {
                    ui.group(|ui| {
                        ui.heading("作者只读预览");
                        ui.label(format!(
                            "{} 项公开条目 · {} 个静态文件 · {} B 原始文件 · {} B ZIP",
                            reviewed.preview.included.len(),
                            reviewed.files.len(),
                            reviewed.raw_bytes,
                            reviewed.zip.len()
                        ));
                        ui.label(format!(
                            "{} 项未纳入报告；预览计划 {} · 文件逐字节解包核对通过",
                            reviewed.preview.exclusions.len(),
                            reviewed.preview.plan_digest
                        ));
                        ui.label("只有下列公开索引内容会进入站点；排除报告仅供作者核对，不写入包。");
                        let mut confirmed = state.confirmed;
                        if ui.checkbox(
                            &mut confirmed,
                            "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
                        ).changed() {
                            state.confirmed = confirmed;
                        }
                        ui.horizontal(|ui| {
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.add_enabled(state.confirmed, egui::Button::new("发布 ZIP")).clicked() {
                                action = Some(PublishAction::Publish);
                            }
                            #[cfg(target_arch = "wasm32")]
                            if ui.add_enabled(state.confirmed, egui::Button::new("下载阅读包")).clicked() {
                                action = Some(PublishAction::Publish);
                            }
                            if ui.button("取消发布").clicked() {
                                action = Some(PublishAction::Cancel);
                            }
                        });
                        ui.separator();
                        for (title, url, text) in reviewed.public_pages.iter().take(8) {
                            ui.label(RichText::new(title).strong());
                            ui.label(format!("{url} · {text}"));
                        }
                        if reviewed.public_pages.len() > 8 {
                            ui.label(format!("另有 {} 页……", reviewed.public_pages.len() - 8));
                        }
                        egui::CollapsingHeader::new(format!(
                            "查看排除明细（{} 项；仅供作者核对）",
                            reviewed.preview.exclusions.len()
                        ))
                        .default_open(false)
                        .show(ui, |ui| {
                            for exclusion in &reviewed.preview.exclusions {
                                ui.label(RichText::new(format!(
                                    "排除：{} · {}",
                                    exclusion.reason_code,
                                    exclusion.source_path.as_deref().unwrap_or("未公开内容")
                                )).color(crate::theme::MUTED));
                            }
                        });
                    });
                } else {
                    ui.label("尚未生成预览；未选任何内容时不会创建空包。");
                }

                ui.horizontal(|ui| {
                    #[cfg(target_arch = "wasm32")]
                    let busy = false;
                    #[cfg(not(target_arch = "wasm32"))]
                    let busy = state.job.is_some();
                    if ui
                        .add_enabled(!busy && state.has_selection(), egui::Button::new("生成 / 更新预览"))
                        .clicked()
                    {
                        action = Some(PublishAction::Preview(state.selection()));
                    }
                    if busy && ui.button("取消生成").clicked() {
                        action = Some(PublishAction::Cancel);
                    }
                    if ui.button("取消发布").clicked() {
                        action = Some(PublishAction::Cancel);
                    }
                });
            });
        self.reader_publish.open = open;
        if !open {
            self.reader_publish = ReaderPublishState::default();
            return;
        }

        match action {
            Some(PublishAction::Preview(selection)) => self.start_reader_publish_preview(selection),
            Some(PublishAction::Cancel) => self.reader_publish = ReaderPublishState::default(),
            Some(PublishAction::Publish) => self.publish_reader_package(),
            #[cfg(not(target_arch = "wasm32"))]
            Some(PublishAction::Browse) => self.choose_reader_package_destination(),
            None => {}
        }
    }

    fn start_reader_publish_preview(&mut self, selection: ReaderExportSelection) {
        self.reader_publish.invalidate_review();
        self.reader_publish.status = Some("正在按 core 公开清单生成静态阅读包……".into());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = self.project.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            std::thread::spawn(move || {
                let result = build_reviewed_package_with_progress(
                    &project,
                    selection,
                    |stage| {
                        let _ = sender.send(ReaderPublishMessage::Stage(stage));
                    },
                    || worker_cancel.load(std::sync::atomic::Ordering::Acquire),
                );
                if !worker_cancel.load(std::sync::atomic::Ordering::Acquire) {
                    let _ = sender.send(ReaderPublishMessage::Done(Box::new(result)));
                }
            });
            self.reader_publish.job = Some(ReaderPublishJob { cancel, receiver });
        }
        #[cfg(target_arch = "wasm32")]
        {
            match build_reviewed_package(&self.project, selection) {
                Ok(reviewed) => {
                    self.reader_publish.status = Some("静态包已生成并逐文件核对。".into());
                    self.reader_publish.reviewed = Some(reviewed);
                }
                Err(error) => self.reader_publish.status = Some(format!("预览失败：{error}")),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_reader_publish_job(&mut self) {
        let Some(job) = self.reader_publish.job.as_ref() else {
            return;
        };
        let messages = job.receiver.try_iter().collect::<Vec<_>>();
        for message in messages {
            match message {
                ReaderPublishMessage::Stage(status) => {
                    self.reader_publish.status = Some(status.into());
                }
                ReaderPublishMessage::Done(result) => {
                    self.reader_publish.job = None;
                    match *result {
                        Ok(reviewed) => {
                            self.reader_publish.reviewed = Some(reviewed);
                            self.reader_publish.status =
                                Some("静态包预览已生成；尚未写入目标或启动下载。".into());
                        }
                        Err(error) => {
                            self.reader_publish.status = Some(format!("预览失败：{error}"));
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_reader_publish_job(&mut self) {}

    fn publish_reader_package(&mut self) {
        let Some(reviewed) = self.reader_publish.reviewed.as_ref() else {
            self.reader_publish.status = Some("请先生成并核对预览。".into());
            return;
        };
        if !self.reader_publish.confirmed {
            self.reader_publish.status = Some("发布前必须明确确认预览范围。".into());
            return;
        }
        let selection = reviewed.selection.clone();
        let expected = reviewed.preview.plan_digest.clone();
        let zip = reviewed.zip.clone();
        let fresh = match self.project.preview_reader_export(&selection) {
            Ok(preview) => preview,
            Err(error) => {
                self.reader_publish.status = Some(format!("发布前复核失败：{error}"));
                self.reader_publish.confirmed = false;
                return;
            }
        };
        if fresh.plan_digest != expected {
            self.reader_publish.invalidate_review();
            self.reader_publish.status = Some("预览已过期；内容变化后请重新预览并确认。".into());
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let target = PathBuf::from(self.reader_publish.destination.trim());
            if target
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
            {
                self.reader_publish.status = Some("目标文件扩展名必须为 .zip。".into());
                return;
            }
            match super::package::write_package_file(&self.project.root, &target, &zip) {
                Ok(()) => {
                    self.reader_publish.status = Some(format!(
                        "阅读包已写入 {}（{} B；工程保存状态未改变）。",
                        target.display(),
                        zip.len()
                    ));
                    self.message = Some("静态阅读 ZIP 已发布；作者工程未改变".into());
                }
                Err(error) => self.reader_publish.status = Some(format!("发布失败：{error}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            match crate::web::download("worldedit-reader-site.zip", &zip, "application/zip") {
                Ok(()) => {
                    self.reader_publish.status = Some(
                        "浏览器下载已启动；文件名 worldedit-reader-site.zip。工程保存状态未改变。"
                            .into(),
                    );
                    self.message = Some("静态阅读 ZIP 下载已启动；作者工程未改变".into());
                }
                Err(error) => self.reader_publish.status = Some(format!("下载失败：{error}")),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn choose_reader_package_destination(&mut self) {
        let initial = PathBuf::from(self.reader_publish.destination.trim());
        let parent = initial
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| self.project.root.parent().unwrap_or(&self.project.root));
        let file_name = initial
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("reader-site.zip");
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(parent)
            .set_file_name(file_name)
            .add_filter("离线阅读 ZIP", &["zip"])
            .save_file()
        {
            self.reader_publish.destination = path.to_string_lossy().into_owned();
            self.reader_publish.confirmed = false;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for ReaderPublishJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(target_arch = "wasm32")]
fn build_reviewed_package(
    project: &Project,
    selection: ReaderExportSelection,
) -> Result<ReviewedPackage, String> {
    build_reviewed_package_with_progress(project, selection, |_| {}, || false)
}

fn build_reviewed_package_with_progress(
    project: &Project,
    selection: ReaderExportSelection,
    mut report_progress: impl FnMut(&'static str),
    is_cancelled: impl Fn() -> bool,
) -> Result<ReviewedPackage, String> {
    report_progress("正在计算 core 预览与排除报告……");
    let preview = project.preview_reader_export(&selection)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    report_progress("正在构建仅包含已选内容的离线站点……");
    let files = project.build_reader_export(&selection, &preview.plan_digest)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    let raw_bytes = files.values().map(Vec::len).sum();
    report_progress("正在编码并逐文件核对 ZIP……");
    let zip = archive::encode(&files)?;
    let decoded = archive::decode(&zip)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    if decoded != files {
        return Err("静态包 ZIP 解包核对与预览文件不一致".into());
    }
    let search_index = files
        .get(&PathBuf::from("search-index.json"))
        .ok_or("静态包缺少公开搜索索引")?;
    let search_entries: Vec<SearchPreviewEntry> = serde_json::from_slice(search_index)
        .map_err(|error| format!("公开搜索索引格式无效：{error}"))?;
    let public_pages = search_entries
        .into_iter()
        .map(|entry| (entry.title, entry.url, entry.text))
        .collect();
    Ok(ReviewedPackage {
        selection,
        preview,
        files,
        zip,
        public_pages,
        raw_bytes,
    })
}

#[derive(serde::Deserialize)]
struct SearchPreviewEntry {
    title: String,
    url: String,
    text: String,
}
