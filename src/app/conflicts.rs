//! 外部文件冲突的只读三方快照查看器。
//!
//! 快照在打开窗口前由调用方捕获；本模块只渲染已捕获的字节，不在每帧
//! 读取磁盘，也不提供覆盖、合并或自动修复动作。
use crate::theme::{self, *};
use egui::RichText;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;
use worldline_core::project::{ConflictSnapshot, Project};

#[derive(Default)]
pub(super) struct ConflictView {
    open: bool,
    workspace_root: PathBuf,
    snapshots: Vec<ConflictSnapshot>,
    selected: usize,
    captured_at: Option<Instant>,
    error: Option<String>,
}

impl ConflictView {
    /// 读取一次当前冲突快照；后续 `show` 调用只使用内存中的字节。
    pub(super) fn capture(project: &Project) -> Self {
        match project.conflict_snapshots() {
            Ok(snapshots) => Self {
                open: true,
                workspace_root: project.root.clone(),
                snapshots,
                selected: 0,
                captured_at: Some(Instant::now()),
                error: None,
            },
            Err(error) => Self {
                open: true,
                workspace_root: project.root.clone(),
                captured_at: Some(Instant::now()),
                error: Some(error),
                ..Self::default()
            },
        }
    }

    pub(super) fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("外部冲突 · 三方差异")
            .id(egui::Id::new("external-conflicts"))
            .open(&mut open)
            .default_width(980.0)
            .default_height(680.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("外部修改与本地缓冲冲突");
                ui.label(theme::muted(
                    "以下内容是一次只读快照。窗口不会覆盖或合并任何一方；若磁盘再次变化，请关闭后重新打开冲突查看。",
                ));
                if let Some(captured_at) = self.captured_at {
                    ui.label(theme::muted(capture_label(captured_at)));
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ERROR, format!("无法读取冲突快照：{error}"));
                    return;
                }
                if self.snapshots.is_empty() {
                    ui.add_space(18.0);
                    ui.label(theme::muted("没有可显示的冲突快照。"));
                    return;
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("冲突文件").strong());
                        egui::ScrollArea::vertical()
                            .id_salt("conflict-file-list")
                            .max_height(540.0)
                            .show(ui, |ui| {
                                for (index, snapshot) in self.snapshots.iter().enumerate() {
                                    let label = relative_path(
                                        &self.workspace_root,
                                        snapshot.path.as_path(),
                                    );
                                    if ui
                                        .add_sized(
                                            [250.0, 34.0],
                                            egui::Button::selectable(
                                                self.selected == index,
                                                RichText::new(label).size(12.0),
                                            ),
                                        )
                                        .on_hover_text(snapshot.path.display().to_string())
                                        .clicked()
                                    {
                                        self.selected = index;
                                    }
                                }
                            });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        if let Some(snapshot) = self.snapshots.get(self.selected) {
                            show_snapshot(ui, &self.workspace_root, snapshot);
                        }
                    });
                });
            });
        self.open = open;
    }
}

fn show_snapshot(ui: &mut egui::Ui, root: &Path, snapshot: &ConflictSnapshot) {
    ui.set_min_width(660.0);
    ui.label(
        RichText::new(relative_path(root, &snapshot.path))
            .strong()
            .size(17.0),
    );
    ui.label(theme::muted(change_summary(snapshot)));
    ui.add_space(8.0);
    ui.columns(3, |columns| {
        show_side(
            &mut columns[0],
            "保存基线",
            "最近一次成功保存的字节",
            snapshot.baseline.as_deref(),
        );
        show_side(
            &mut columns[1],
            "本地缓冲",
            "当前工程中保留的字节",
            snapshot.local.as_deref(),
        );
        show_side(
            &mut columns[2],
            "磁盘版本",
            "捕获时从工作区读取的字节",
            snapshot.disk.as_deref(),
        );
    });
}

fn show_side(ui: &mut egui::Ui, title: &str, subtitle: &str, bytes: Option<&[u8]>) {
    ui.label(RichText::new(title).strong());
    ui.label(theme::muted(subtitle));
    ui.add_space(4.0);
    match bytes {
        None => {
            ui.colored_label(GOLD, "缺失：此方没有文件（删除或尚不存在）");
        }
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => {
                // &str 是只读文本缓冲，允许选择复制但不会改变快照字节。
                let mut text = text;
                egui::ScrollArea::vertical()
                    .id_salt(("conflict-side", title))
                    .max_height(480.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(18)
                                .desired_width(f32::INFINITY),
                        );
                    });
            }
            Err(_) => {
                ui.colored_label(
                    ERROR,
                    format!("非 UTF-8：无法按 UTF-8 解码（{} 字节）", bytes.len()),
                );
                ui.label(theme::muted(format!(
                    "原始字节（十六进制）：{}",
                    hex_preview(bytes)
                )));
            }
        },
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn change_summary(snapshot: &ConflictSnapshot) -> String {
    let local = if snapshot.local == snapshot.baseline {
        "本地与基线相同"
    } else {
        "本地已变化"
    };
    let disk = if snapshot.disk == snapshot.baseline {
        "磁盘与基线相同"
    } else {
        "磁盘已变化"
    };
    format!("{local} · {disk} · 只读查看")
}

fn hex_preview(bytes: &[u8]) -> String {
    let shown = bytes.len().min(256);
    let mut output = bytes[..shown]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if shown < bytes.len() {
        output.push_str(" …（仅显示前 256 字节）");
    }
    output
}

fn capture_label(captured_at: Instant) -> String {
    format!(
        "本次刷新捕获的只读快照 · 已经过 {} 秒；关闭后再次打开冲突查看可重新读取。",
        captured_at.elapsed().as_secs()
    )
}

#[cfg(test)]
mod tests {
    use super::show_side;

    #[test]
    fn rendering_snapshot_text_does_not_change_captured_bytes() {
        let text = b"event local\n  -> END\n".to_vec();
        let invalid = vec![b'{', 0xff, b'}'];
        let before_text = text.clone();
        let before_invalid = invalid.clone();
        let context = egui::Context::default();
        let _ = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_side(ui, "本地缓冲", "测试", Some(&text));
                show_side(ui, "磁盘版本", "测试", Some(&invalid));
            });
        });
        assert_eq!(text, before_text);
        assert_eq!(invalid, before_invalid);
    }
}
