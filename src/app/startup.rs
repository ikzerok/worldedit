//! 原生启动与工作区选择。欢迎状态没有可保存的工程，也不隐式创建目录。
#[cfg(test)]
mod tests;
mod welcome;
mod workspace_dialog;

use super::{Tab, WorldeditApp};
use std::path::{Path, PathBuf};
use worldline_core::project::Project;

pub(super) use workspace_dialog::{WorkspaceDialog, WorkspaceOperation};

#[derive(Default)]
pub(super) struct StartupState {
    pub welcome: bool,
    pub dialog: Option<WorkspaceDialog>,
    ime: bool,
}

pub(super) fn recent_project(storage: Option<&dyn eframe::Storage>) -> Option<PathBuf> {
    // 记录只用于尝试打开；失效路径保留以显示错误，绝不将存储当作创建授权。
    super::personal::PersonalState::restore(storage).last_project()
}

pub(super) fn preferred_project(
    storage: Option<&dyn eframe::Storage>,
    explicit: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit.or_else(|| recent_project(storage))
}

impl WorldeditApp {
    pub fn start_native(cc: &eframe::CreationContext<'_>, explicit: Option<PathBuf>) -> Self {
        let selected = preferred_project(cc.storage, explicit);
        let mut app = Self::new(cc, selected);
        app.native_startup.welcome = !app.saved_location;
        app
    }

    /// 仅在作者确认“创建作品”后调用。校验和保存失败都保留当前工程与输入。
    pub(super) fn create_native_project(&mut self, root: &Path) -> bool {
        self.accept_created_project(create_project(root))
    }

    fn accept_created_project(&mut self, result: Result<Project, String>) -> bool {
        match result {
            Ok(project) => {
                self.project = project;
                self.active_file = self.project.entry.clone();
                self.saved_location = true;
                self.native_startup.welcome = false;
                self.native_startup.dialog = None;
                self.reset_views();
                self.recompile();
                self.tab = Tab::Manuscript;
                self.message = Some("已创建空白作品，可以开始写作".into());
                true
            }
            Err(error) => {
                self.io_error = Some(error);
                false
            }
        }
    }

    fn workspace_input_composing(&mut self, ctx: &egui::Context) -> bool {
        let mut ime_frame = false;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Ime(event) = event {
                    ime_frame = true;
                    self.native_startup.ime =
                        matches!(event, egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_));
                }
            }
        });
        self.native_startup.ime || ime_frame
    }
}

fn create_project(root: &Path) -> Result<Project, String> {
    if !root.is_absolute() {
        return Err("请填写新工作区文件夹的绝对完整路径".into());
    }
    match std::fs::read_dir(root) {
        Ok(mut entries) => match entries.next() {
            None => {}
            Some(Ok(_)) => {
                return Err("新建作品请选择空文件夹，或填写尚不存在的新文件夹路径".into());
            }
            Some(Err(error)) => return Err(format!("无法读取 {}：{error}", root.display())),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("无法访问 {}：{error}", root.display())),
    }
    let mut project = Project::new(root);
    project
        .save()
        .map_err(|error| format!("无法创建 {}：{error}", root.display()))?;
    Ok(project)
}
