//! 地图页的“新建空白地图”表单。
//!
//! 表单只负责收集用户输入和调用 core 的公开命令边界；清单、地图 JSON
//! 的结构与校验全部由 `worldline-core::map_creation` 处理。

use std::collections::BTreeMap;

use worldline_core::map_creation::{
    self, CreateMapCommand, CreateMapRequest, MISSING_DOCUMENT_HASH,
};
use worldline_core::presentation_commands::document_hash;
use worldline_core::presentation_commands::Revision;

/// 新建地图表单的本地输入。
///
/// 尺寸保留为字符串，提交失败时可以原样保留用户输入，避免一次错误把表单
/// 变回默认值。
#[derive(Clone, Debug, Default)]
pub(super) struct MapCreationForm {
    pub(super) open: bool,
    pub(super) id: String,
    pub(super) title: String,
    pub(super) width: String,
    pub(super) height: String,
    expected_revision: Option<Revision>,
    expected_manifest: Option<String>,
}

impl MapCreationForm {
    pub(super) fn open_with_defaults(
        &mut self,
        expected_revision: Revision,
        expected_manifest: String,
    ) {
        self.open = true;
        self.expected_revision = Some(expected_revision);
        self.expected_manifest = Some(expected_manifest);
        if self.id.is_empty() {
            self.id = "map".into();
        }
        if self.title.is_empty() {
            self.title = "新地图".into();
        }
        if self.width.is_empty() {
            self.width = "2048".into();
        }
        if self.height.is_empty() {
            self.height = "1536".into();
        }
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.expected_revision = None;
        self.expected_manifest = None;
    }

    fn request(&self) -> Result<CreateMapRequest, String> {
        let width = self
            .width
            .trim()
            .parse::<u32>()
            .map_err(|_| "地图宽度必须是正整数".to_string())?;
        let height = self
            .height
            .trim()
            .parse::<u32>()
            .map_err(|_| "地图高度必须是正整数".to_string())?;
        Ok(CreateMapRequest::new(
            self.id.trim(),
            self.title.trim(),
            width,
            height,
        ))
    }
}

impl super::WorldeditApp {
    /// 在地图页的右侧展示显式创建入口。
    ///
    /// root 在 `map_tab` 的右侧面板中调用此方法，并在 `WorldeditApp` 中持有
    /// 一个 `MapCreationForm` 字段。入口只在“编辑展示”模式出现；浏览模式
    /// 仍是零写入。
    pub(super) fn map_creation_panel(&mut self, ui: &mut egui::Ui) {
        if !self.map_canvas.is_edit_mode() {
            return;
        }
        ui.separator();
        ui.label(egui::RichText::new("地图工作区").strong());
        if ui.button("新建空白地图").clicked() {
            self.map_creation
                .open_with_defaults(self.map_revision, self.map_manifest_baseline());
        }
        if !self.map_creation.open {
            return;
        }

        let mut submit = false;
        let mut cancel = false;
        ui.group(|ui| {
            ui.label(egui::RichText::new("创建地图").strong());
            ui.label(crate::theme::muted(
                "创建会注册一个空白地图和地点图层；创建后可添加栅格图层和内容引用，不要求启动试玩。",
            ));
            ui.label("ID");
            ui.text_edit_singleline(&mut self.map_creation.id);
            ui.label("名称");
            ui.text_edit_singleline(&mut self.map_creation.title);
            ui.horizontal(|ui| {
                ui.label("画布宽度");
                ui.text_edit_singleline(&mut self.map_creation.width);
            });
            ui.horizontal(|ui| {
                ui.label("画布高度");
                ui.text_edit_singleline(&mut self.map_creation.height);
            });
            ui.horizontal(|ui| {
                submit = ui.button("创建").clicked();
                cancel = ui.small_button("取消").clicked();
            });
        });
        if cancel {
            self.map_creation.close();
        } else if submit {
            self.create_map_from_form();
        }
    }

    /// 提交当前新建地图表单。
    ///
    /// 失败只更新错误提示，输入仍留在 `MapCreationForm` 中；成功后把新地图
    /// 选中、刷新只读展示缓存，并将整个两文档变更记为一次 Project 历史项。
    pub(super) fn create_map_from_form(&mut self) -> bool {
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式".into());
            return false;
        }
        let request = match self.map_creation.request() {
            Ok(request) => request,
            Err(error) => {
                self.io_error = Some(error);
                return false;
            }
        };

        let manifest = self.project.root.join(".world/project.json");
        let mut expected_documents = BTreeMap::new();
        expected_documents.insert(
            manifest,
            self.map_creation
                .expected_manifest
                .clone()
                .unwrap_or_else(|| MISSING_DOCUMENT_HASH.into()),
        );
        let before = self.project.clone();
        let command = CreateMapCommand {
            expected_revision: self
                .map_creation
                .expected_revision
                .unwrap_or(self.map_revision),
            expected_documents,
            request,
        };
        match map_creation::apply(&mut self.project, &mut self.map_revision, command) {
            Ok(result) => {
                self.remember(before);
                self.map_selection = Some(result.map_id);
                self.map_canvas.reset_local_preview();
                self.refresh_presentation_after_map_command();
                self.map_creation.close();
                self.io_error = None;
                self.message = Some("已创建空白地图（可撤销）".into());
                true
            }
            Err(error) => {
                self.io_error = Some(error.to_string());
                false
            }
        }
    }

    fn map_manifest_baseline(&self) -> String {
        let manifest = self.project.root.join(".world/project.json");
        self.project
            .authoring_document(&manifest)
            .ok()
            .filter(|document| !document.is_deleted())
            .map(|document| document_hash(document.bytes()))
            .unwrap_or_else(|| MISSING_DOCUMENT_HASH.into())
    }
}
