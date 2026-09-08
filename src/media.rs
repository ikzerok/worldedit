//! 文件引用入口；引用的存储与相对路径转换由节点编辑流程负责。
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const OPEN_REFERENCE_LABEL: &str = "用系统应用打开";
#[cfg(target_arch = "wasm32")]
pub(crate) const OPEN_REFERENCE_LABEL: &str = "下载引用文件";

/// 返回用户选中的文件。取消选择时返回空列表，不读取或打开文件内容。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pick_reference_files(
    ui: &mut egui::Ui,
    _target: &worldline_core::catalog::TargetRef,
) -> Vec<PathBuf> {
    if ui
        .add(crate::theme::primary("添加引用文件"))
        .on_hover_text("选择图片、音频或其他设定资料，可一次选择多个文件")
        .clicked()
    {
        rfd::FileDialog::new()
            .set_title("添加引用文件")
            .pick_files()
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// 只在作者点击打开后调用系统关联应用;路径作为独立参数传递。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_reference(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let path = worldline_core::file_access::within(root, path)?;
    if !path.is_file() {
        return Err("引用文件不存在".into());
    }
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开文件:{e}"))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn pick_reference_files(
    ui: &mut egui::Ui,
    target: &worldline_core::catalog::TargetRef,
) -> Vec<PathBuf> {
    if ui.add(crate::theme::primary("添加引用文件")).clicked() {
        crate::web::select_files(
            ui.ctx(),
            false,
            "",
            crate::web::FileAction::Attach(target.clone()),
        );
    }
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_reference(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let path = worldline_core::file_access::within(root, path)?;
    let bytes = worldline_core::file_access::read(&path).map_err(|e| e.to_string())?;
    crate::web::download(
        path.file_name()
            .and_then(|p| p.to_str())
            .unwrap_or("reference"),
        &bytes,
        "application/octet-stream",
    )
}
