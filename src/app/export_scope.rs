//! 目录和 ZIP 共用的已应用快照决定点；从不应用、清空或保存作者草稿。
mod inventory;
use super::{Tab, WorldeditApp};
use crate::theme;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[derive(Clone)]
pub(super) enum ExportDestination {
    #[cfg(not(target_arch = "wasm32"))]
    Directory(PathBuf),
    #[cfg(not(target_arch = "wasm32"))]
    Zip(PathBuf),
    #[cfg(target_arch = "wasm32")]
    BrowserZip,
}
impl ExportDestination {
    fn path(&self) -> &std::path::Path {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Directory(path) | Self::Zip(path) => path,
            #[cfg(target_arch = "wasm32")]
            Self::BrowserZip => std::path::Path::new("worldedit-export.zip"),
        }
    }
}

pub(super) struct ExportConfirmation {
    destination: ExportDestination,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnappliedInput {
    pub(in crate::app) kind: &'static str,
    pub(in crate::app) source: String,
}

impl WorldeditApp {
    pub(super) fn request_strict_export(&mut self, destination: ExportDestination) -> bool {
        if self.unapplied_export_inputs().is_empty() {
            self.write_strict_export(&destination)
        } else {
            self.export_confirmation = Some(ExportConfirmation { destination });
            true
        }
    }

    fn write_strict_export(&mut self, destination: &ExportDestination) -> bool {
        let result = match destination {
            #[cfg(not(target_arch = "wasm32"))]
            ExportDestination::Directory(path) => self.project.export(path),
            #[cfg(not(target_arch = "wasm32"))]
            ExportDestination::Zip(path) => super::package::export_package_bytes(&self.project)
                .and_then(|bytes| {
                    super::package::write_package_file(&self.project.root, path, &bytes)
                }),
            #[cfg(target_arch = "wasm32")]
            ExportDestination::BrowserZip => self.write_browser_export(),
        };
        match result {
            Ok(()) => {
                self.io_error = None;
                let action = if cfg!(target_arch = "wasm32") {
                    "已请求下载"
                } else {
                    "已导出"
                };
                self.message = Some(format!(
                    "{action}已应用工程快照：{}（未应用输入保留，当前保存状态未改变）",
                    destination.path().display()
                ));
                true
            }
            Err(error) => {
                self.io_error = Some(error);
                false
            }
        }
    }

    pub(in crate::app) fn return_to_export_input(&mut self, kind: &str) {
        self.tab = match kind {
            "事件正文与分支" | "时段资料" => Tab::Timeline,
            "人物资料" => Tab::Characters,
            "世界观" => Tab::World,
            "标签" | "状态" | "锚点" | "实体资料" | "语义关系" | "关系类型" => {
                Tab::Catalog
            }
            "Wiki词条" => Tab::Wiki,
            "书稿 / 正文草稿" => Tab::Manuscript,
            "地图草稿" | "新建地图" => Tab::Map,
            "本地化草稿" => Tab::Localization,
            "审阅批注" | "审阅提案" => Tab::Review,
            "工程模板草稿" => Tab::Templates,
            "共享网络布局" => Tab::Network,
            "共享查询定义" => Tab::Catalog,
            "文件名称" | "源码路径" | "正在输入的源码 / 输入法" => Tab::Edit,
            _ => self.tab,
        };
        if kind == "共享查询定义" {
            self.catalog_workbench.open = true;
        }
        if kind == "持续资料约束草稿" {
            self.schema_ui.open = true;
        }
        if kind == "正文替换预览" {
            self.search_open = true;
        }
    }

    pub(super) fn export_scope_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirmation) = self.export_confirmation.take() else {
            return;
        };
        // 不复用打开窗口时的脏状态；包括窗口隐藏后保留的输入和最新外部刷新。
        let inputs = self.unapplied_export_inputs();
        let mut cancel = false;
        let mut proceed = false;
        let mut return_to = None;
        #[cfg(not(target_arch = "wasm32"))]
        let mut edit_target = false;
        egui::Modal::new(egui::Id::new("export-applied-snapshot")).show(ctx, |ui| {
            ui.set_width(590.0);
            ui.heading("导出已应用版，不含以下未应用输入");
            ui.label("已应用但尚未保存的工程修改会进入副本。以下输入仍留在编辑器中，不会自动应用、丢弃或保存。");
            ui.label(format!("目标：{}", confirmation.destination.path().display()));
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for input in &inputs {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} · {}", input.kind, input.source));
                        if ui.small_button("返回处理").clicked() {
                            return_to = Some(input.kind);
                        }
                    });
                }
            });
            if inputs.is_empty() {
                ui.label("当前已无未应用输入；继续仍校验全部工程文件。");
            }
            ui.label("包含全部工程文件、未引用附件与展示文档；仍执行严格校验，导出副本不会解除原工作区冲突。");
            ui.horizontal_wrapped(|ui| {
                proceed = ui.add(theme::primary("明确继续：仅导出已应用版")).clicked();
                cancel = ui.button("取消导出").clicked();
                #[cfg(not(target_arch = "wasm32"))]
                { edit_target = ui.button("修改目标路径").clicked(); }
            });
        });
        #[cfg(not(target_arch = "wasm32"))]
        if edit_target {
            let operation = match &confirmation.destination {
                ExportDestination::Directory(_) => super::DirectoryOperation::ExportDirectory,
                ExportDestination::Zip(_) => super::DirectoryOperation::ExportZip,
            };
            self.directory = Some(super::DirectoryDialog {
                path: confirmation.destination.path().display().to_string(),
                operation,
            });
            return;
        }
        if let Some(kind) = return_to {
            self.return_to_export_input(kind);
        } else if !cancel && !(proceed && self.write_strict_export(&confirmation.destination)) {
            self.export_confirmation = Some(confirmation);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod zip_target_tests;
