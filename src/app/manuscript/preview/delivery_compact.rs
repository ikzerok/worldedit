//! 低高度渐进展开：完整范围/交付菜单不占材料正文的固定高度。
use crate::{app::WorldeditApp, theme};
use std::sync::Arc;
use worldline_core::manuscript::ManuscriptDeliveryReport;

pub(super) fn controls(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
) -> Option<(Arc<ManuscriptDeliveryReport>, bool, Option<String>)> {
    let blocker = app.review_input_blocker(ui.ctx());
    let busy = app.manuscript.preview_cache.delivery.job.is_some();
    ui.horizontal(|ui| {
        egui::containers::menu::MenuButton::new("范围与交付")
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).clamp(1.0, 600.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let output = egui::ScrollArea::vertical()
                    .id_salt("compact-manuscript-delivery-controls")
                    .max_height((ui.ctx().screen_rect().height() - 48.0).max(1.0))
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        let _ = super::delivery_view::draw_header(app, ui);
                    });
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("delivery-menu-viewport"), output.inner_rect)
                });
                #[cfg(not(test))]
                let _ = output;
            });
        if busy {
            if ui.button("取消生成").clicked() {
                app.cancel_manuscript_delivery();
            }
        } else if theme::add_enabled(ui, blocker.is_none(), egui::Button::new("生成审稿")).clicked()
        {
            app.begin_manuscript_delivery(ui.ctx());
        }
        let state = &app.manuscript.preview_cache.delivery;
        let caption =
            if let Some(progress) = state.job.as_ref().and_then(|job| job.progress.as_ref()) {
                format!("{} / {} 章", progress.completed, progress.total)
            } else if busy {
                "生成中".into()
            } else if let Some(report) = &state.reviewed {
                format!(
                    "{} · {}章",
                    if app.manuscript_delivery_is_current() {
                        "静态"
                    } else {
                        "过期"
                    },
                    report.scope().selected_occurrences
                )
            } else {
                "待生成".into()
            };
        if ui.available_width() > 24.0 {
            ui.add(egui::Label::new(theme::muted(caption)).truncate())
                .on_hover_text(
                    state.notice.as_deref().unwrap_or(
                        "作者私密全分支审稿；在“范围与交付”核对完整筛选、来源和交付动作",
                    ),
                );
        }
    });
    let report = app.manuscript.preview_cache.delivery.reviewed.clone()?;
    let current = app.manuscript_delivery_is_current();
    if !current {
        app.manuscript.preview_cache.delivery.confirmed = false;
    }
    Some((report, current, blocker))
}
