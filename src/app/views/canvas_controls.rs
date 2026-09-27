use super::super::WorldeditApp;
use crate::theme::{self, *};
use egui::Vec2;

pub(super) fn header(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    timeline: bool,
    storyline_count: usize,
    event_count: usize,
    document_count: usize,
    visit_coverage: bool,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(if timeline {
                "时间线"
            } else {
                "事件关系图"
            });
            ui.label(theme::muted(format!(
                "{} 条故事线  /  {} 个事件  /  {} 个文件",
                storyline_count, event_count, document_count
            )));
            if visit_coverage {
                ui.label(theme::muted(
                    "访问覆盖标记：数字表示实际访问次数；无标记表示未测试",
                ));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme::primary("＋ 新建事件")).clicked() {
                app.new_event(None);
            }
            ui.add(
                egui::TextEdit::singleline(&mut app.search)
                    .hint_text("搜索 ID / 名称 / 人物")
                    .desired_width(160.0),
            );
        });
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.small_button("＋ 新建时段").clicked() {
            app.new_period_dialog();
        }
        if let Some(from) = app.link_from.clone() {
            ui.colored_label(ACCENT, format!("从 {from} 连线 · 点击目标事件"));
            ui.label(theme::muted("选择文案"));
            ui.add(
                egui::TextEdit::singleline(&mut app.link_label)
                    .desired_width(110.0)
                    .hint_text("留空设置直接出口"),
            );
            if ui.small_button("取消").clicked() {
                app.link_from = None;
            }
        } else {
            ui.label(theme::muted(if timeline {
                "拖动卡片调整顺序与故事线 · 点击编辑 · 双击空白处添加"
            } else {
                "拖动卡片整理布局 · 拖动右侧圆点建立连接"
            }));
        }
    });
    ui.add_space(12.0);
}

pub(super) fn footer(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    viewport: Vec2,
    width: f32,
    height: f32,
) {
    ui.horizontal(|ui| {
        ui.colored_label(ACCENT, "— 选择");
        ui.colored_label(GOLD, "— 跃迁");
        ui.colored_label(BLUE, "— 跨线漂流");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("＋").clicked() {
                app.zoom = (app.zoom + 0.1).min(1.6);
            }
            ui.label(theme::muted(format!("{:.0}%", app.zoom * 100.0)));
            if ui.small_button("−").clicked() {
                app.zoom = (app.zoom - 0.1).max(0.5);
            }
            if ui.small_button("定位所选").clicked() {
                app.focus_event = app.event_editor.as_ref().map(|e| e.draft.id.clone());
            }
            if ui.small_button("适配全图").clicked() {
                app.zoom = (viewport.x / width.max(1.0))
                    .min(viewport.y / height.max(1.0))
                    .clamp(0.25, 1.6);
            }
            if ui.small_button("重置布局").clicked() {
                app.zoom = 1.0;
                app.graph_positions.clear();
            }
        });
    });
}
