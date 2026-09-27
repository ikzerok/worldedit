//! 桌面 Markdown 迁移向导；解析、冲突与写入都由 worldline-core 承担。
mod apply;
mod input;
mod plan;

use super::WorldeditApp;
use egui::Id;
use std::collections::BTreeMap;
use worldline_core::markdown_import::MarkdownImportPlan;
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::markdown_import::MarkdownImportRequest;
use worldline_core::workspace_snapshot::Files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetMode {
    NewProject,
    CurrentProject,
}

pub(super) struct Wizard {
    source_root: String,
    #[cfg(not(target_arch = "wasm32"))]
    target_root: String,
    target_mode: TargetMode,
    namespace: String,
    id_overrides: BTreeMap<String, String>,
    source_files: Option<Files>,
    accept_losses: bool,
    allow_language_upgrade: bool,
    #[cfg(not(target_arch = "wasm32"))]
    request: Option<MarkdownImportRequest>,
    plan: Option<MarkdownImportPlan>,
    error: Option<String>,
    stale: bool,
    closed: bool,
    page_offset: usize,
    link_offset: usize,
    attachment_offset: usize,
    loss_offset: usize,
    conflict_offset: usize,
    name_conflict_offset: usize,
    file_offset: usize,
    preview_generation: u64,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            source_root: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            target_root: String::new(),
            target_mode: TargetMode::NewProject,
            namespace: String::new(),
            id_overrides: BTreeMap::new(),
            source_files: None,
            accept_losses: false,
            allow_language_upgrade: false,
            #[cfg(not(target_arch = "wasm32"))]
            request: None,
            plan: None,
            error: None,
            stale: false,
            closed: false,
            page_offset: 0,
            link_offset: 0,
            attachment_offset: 0,
            loss_offset: 0,
            conflict_offset: 0,
            name_conflict_offset: 0,
            file_offset: 0,
            preview_generation: 0,
        }
    }
}

impl Wizard {
    pub(super) fn show(&mut self, ctx: &egui::Context, app: &mut WorldeditApp) -> bool {
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.closed = true;
        }
        if self.closed {
            return false;
        }

        let mut open = true;
        egui::Window::new("导入 Markdown")
            .id(Id::new("markdown-import-wizard"))
            .open(&mut open)
            .resizable(true)
            .default_size([680.0, 600.0])
            .show(ctx, |ui| self.contents(ui, app, ctx));
        open && !self.closed
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn set_source_files(&mut self, files: Files) {
        self.source_root = "浏览器所选文件夹".into();
        self.source_files = Some(files);
        self.invalidate_preview();
    }

    fn invalidate_preview(&mut self) {
        self.plan = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.request = None;
        }
        self.error = None;
        self.stale = false;
    }
}

impl WorldeditApp {
    pub(super) fn markdown_import_window(&mut self, ctx: &egui::Context) {
        let Some(mut wizard) = self.markdown_import_wizard.take() else {
            return;
        };
        if wizard.show(ctx, self) {
            self.markdown_import_wizard = Some(wizard);
        }
    }
}
