//! 原生启动先读取同一设备存储，再决定是否需要目录选择。
use super::WorldeditApp;
use std::path::PathBuf;
use worldline_core::project::Project;

pub(super) fn recent_project(storage: Option<&dyn eframe::Storage>) -> Option<PathBuf> {
    let path = super::personal::PersonalState::restore(storage).last_project()?;
    // 存储不是创建授权；只接受已经存在且能够完整打开的工程。
    if !path.exists() || Project::open(&path).is_err() {
        return None;
    }
    Some(path)
}
pub(super) fn preferred_project(
    storage: Option<&dyn eframe::Storage>,
    explicit: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit.or_else(|| recent_project(storage))
}
impl WorldeditApp {
    pub fn start_native(cc: &eframe::CreationContext<'_>, explicit: Option<PathBuf>) -> Self {
        let selected = preferred_project(cc.storage, explicit).or_else(|| {
            rfd::FileDialog::new()
                .set_title("选择工作区目录，空目录将创建空白工程")
                .pick_folder()
        });
        let outcome = selected
            .as_ref()
            .map(|path| {
                if path.is_dir()
                    && std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
                {
                    Project::new(path)
                        .save()
                        .map_err(|error| error.to_string())?;
                }
                Project::open(path)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
            .transpose();
        match (selected, outcome) {
            (Some(path), Ok(_)) => Self::new(cc, Some(path)),
            (_, outcome) => {
                if let Err(error) = outcome {
                    eprintln!("无法打开指定工程：{error}");
                }
                let mut app = Self::new(cc, None);
                app.allow_close = true;
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                app
            }
        }
    }
}
