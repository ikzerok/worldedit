use super::*;
use worldline_runtime::{InspectionBaseline, InspectionGroup};

pub(super) fn render(
    ui: &mut egui::Ui,
    running: &mut Running,
    query_enabled: bool,
    source_enabled: bool,
    composing: bool,
    action: &mut Option<Action>,
) {
    ui.label("此试演自己的真实状态；查询不重新求值、不推进故事");
    let before = running.query.clone();
    ui.add_enabled(
        query_enabled,
        egui::TextEdit::singleline(&mut running.query.text)
            .id(egui::Id::new("draft-rehearsal-inspection-search"))
            .hint_text("查名称、片段或当前值…")
            .char_limit(256)
            .desired_width(f32::INFINITY),
    );
    ui.add_enabled_ui(query_enabled && !composing, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (group, label) in [
                (None, "全部"),
                (Some(InspectionGroup::Global), "全局"),
                (Some(InspectionGroup::Local), "调用局部"),
                (Some(InspectionGroup::State), "状态集"),
            ] {
                ui.selectable_value(&mut running.query.group, group, label);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut running.query.changed_only, "只看变化");
            ui.selectable_value(
                &mut running.query.compare_to,
                InspectionBaseline::First,
                "对比首次",
            );
            ui.selectable_value(
                &mut running.query.compare_to,
                InspectionBaseline::Previous,
                "对比上次",
            );
        });
    });
    if running.query != before {
        running.query.offset = 0;
        running.query.expected_stamp = None;
    }
    if query_enabled && !composing && running.query != running.submitted_query {
        running.submitted_query = running.query.clone();
        *action = Some(Action::Inspect {
            query: running.query.clone(),
        });
    }
    let Some(page) = &running.view.inspection else {
        ui.label("尚无可返回的真实检查页");
        return;
    };
    ui.label(format!(
        "{} · 匹配 {} / {} 项 · 不可比较 {} 项",
        super::super::super::inspection::status(page.status),
        page.total_matches,
        page.total_items,
        page.incomparable_items
    ));
    ui.label(super::super::super::inspection::observation_note(page));
    if page.history_omitted {
        ui.colored_label(
            theme::WARNING(),
            "部分历史值已省略，不能据此补零或推断没有变化",
        );
    }
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(
            ui,
            query_enabled && !composing && page.offset > 0,
            egui::Button::new("上一页"),
        )
        .clicked()
        {
            running.query.offset = page.offset.saturating_sub(page.limit);
            running.query.expected_stamp = Some(page.stamp);
            running.submitted_query = running.query.clone();
            *action = Some(Action::Inspect {
                query: running.query.clone(),
            });
        }
        ui.label(format!(
            "{}–{}",
            if page.items.is_empty() {
                0
            } else {
                page.offset + 1
            },
            page.offset + page.items.len()
        ));
        if theme::add_enabled(
            ui,
            query_enabled && !composing && page.next_offset.is_some(),
            egui::Button::new("下一页"),
        )
        .clicked()
        {
            running.query.offset = page.next_offset.unwrap_or(0);
            running.query.expected_stamp = Some(page.stamp);
            running.submitted_query = running.query.clone();
            *action = Some(Action::Inspect {
                query: running.query.clone(),
            });
        }
    });
    for item in &page.items {
        ui.group(|ui| {
            ui.label(format!(
                "{} · {}{}",
                super::super::super::inspection::group(item.key.group),
                item.key.name,
                item.fragment
                    .as_ref()
                    .map(|name| format!(" · {name} 调用 #{}", item.key.call_id.unwrap_or(0)))
                    .unwrap_or_default()
            ));
            let change = if running.query.compare_to == InspectionBaseline::First {
                item.first_change
            } else {
                item.previous_change
            };
            ui.label(super::super::super::inspection::change_label(change));
            for (caption, cell) in [
                ("首次", &item.first),
                ("上次", &item.previous),
                ("当前", &item.current),
            ] {
                super::super::super::inspection::cell(ui, caption, cell);
            }
            if let Some(source) = &item.source {
                if theme::add_enabled(ui, source_enabled, egui::Button::new("返回此状态声明"))
                    .clicked()
                {
                    *action = Some(Action::DeclarationSource {
                        source: source.clone(),
                    });
                }
                ui.label(theme::muted("定位声明头；不表示最后写入或因果来源"));
            } else {
                ui.label(theme::muted("没有可确认的全局声明来源"));
            }
        });
    }
}
