//! 明确确认后的只读副本交付；磁盘与完整已应用基线在操作时重新核对。
use crate::app::WorldeditApp;

impl WorldeditApp {
    pub(super) fn checked_report_markdown(&self) -> Result<&str, String> {
        let state = &self.playthrough_report;
        let reviewed = state.reviewed.as_ref().ok_or("请先生成并核对试玩报告")?;
        if state.job.is_some() {
            return Err("正在验证，请等待本次结果".into());
        }
        if !state.privacy_confirmed {
            return Err("请先确认报告包含作者私密内容及已展示的范围".into());
        }
        if reviewed.settings != (state.max_steps, state.time_budget_ms) {
            return Err("验证预算已变化，请重新生成报告".into());
        }
        if state.route != reviewed.route {
            return Err("路径选择已变化，请重新生成报告".into());
        }
        if !reviewed.scope.matches_project(&self.project, self.version) {
            return Err("报告已过期，已应用稿或完整内容基线变化；请重新验证".into());
        }
        self.project
            .verify_review_navigation()
            .map_err(|error| format!("工作区观察已过期，请刷新后重新验证：{error}"))?;
        Ok(&reviewed.report.markdown)
    }

    pub(super) fn copy_playthrough_report(&mut self, ctx: &egui::Context) {
        let result = self
            .checked_report_markdown()
            .map(|markdown| ctx.copy_text(markdown.to_owned()));
        self.playthrough_report.notice = Some(match result {
            Ok(()) => "已复制经核对的 Markdown 报告；请谨慎选择接收者".into(),
            Err(error) => error,
        });
    }

    pub(super) fn save_playthrough_report(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let result = self.checked_report_markdown().and_then(|markdown| {
            write_report_file(
                &self.project.root,
                std::path::Path::new(self.playthrough_report.destination.trim()),
                markdown.as_bytes(),
            )
        });
        #[cfg(target_arch = "wasm32")]
        let result = self.checked_report_markdown().and_then(|markdown| {
            crate::web::download(
                "worldedit-playthrough-report.md",
                markdown.as_bytes(),
                "text/markdown;charset=utf-8",
            )
        });
        self.playthrough_report.notice = Some(match result {
            #[cfg(not(target_arch = "wasm32"))]
            Ok(()) => "已保存新 Markdown 报告；当前工程保存状态未改变".into(),
            #[cfg(target_arch = "wasm32")]
            Ok(()) => "已请求浏览器下载 Markdown；当前工程保存状态未改变".into(),
            Err(error) => error,
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn choose_playthrough_report_destination(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("playthrough-report.md")
            .add_filter("Markdown 报告", &["md"])
            .save_file()
        {
            self.playthrough_report.destination = path.to_string_lossy().into_owned();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_report_file(
    root: &std::path::Path,
    target: &std::path::Path,
    bytes: &[u8],
) -> Result<(), String> {
    use std::io::Write;
    if !target.is_absolute() || target.file_name().is_none() {
        return Err("请输入工作区外新 Markdown 文件的绝对完整路径".into());
    }
    let root = crate::app::package::normalized_path(root);
    let target = crate::app::package::normalized_path(target);
    if crate::app::package::is_same_or_descendant(&root, &target) {
        return Err("试玩报告必须保存在当前工作区外".into());
    }
    if target.extension().and_then(|value| value.to_str()) != Some("md") {
        return Err("试玩报告目标必须使用 .md 扩展名".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| format!("无法创建新报告（已有文件不会覆盖）：{error}"))?;
    let result = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        return Err(match std::fs::remove_file(&target) {
            Ok(()) => format!("报告写入失败：{error}"),
            Err(cleanup) => format!("报告写入失败：{error}；不完整文件清理失败：{cleanup}"),
        });
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn native_report_requires_new_external_markdown_and_does_not_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "report-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = root.join("project");
        std::fs::create_dir_all(&workspace).unwrap();
        let target = root.join("report.md");
        assert!(write_report_file(&workspace, std::path::Path::new("report.md"), b"x").is_err());
        assert!(write_report_file(&workspace, &workspace.join("report.md"), b"x").is_err());
        assert!(write_report_file(&workspace, &root.join("report.html"), b"x").is_err());
        write_report_file(&workspace, &target, b"first").unwrap();
        assert!(write_report_file(&workspace, &target, b"second").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        std::fs::remove_dir_all(root).unwrap();
    }
}
