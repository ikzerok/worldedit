//! 时间约束表单只展示 core 的版本、范围与拒绝原因，不自行推导合法边。
use super::super::EventEditor;
use crate::theme::{self, ERROR};
use worldline_core::authoring::{EventPredecessorOption, EventPredecessorOptions};
use worldline_core::project::Project;
use worldline_core::timeline::{PeriodInfo, TemporalOrderScope};

pub(super) fn options(project: &Project, editor: &mut EventEditor) -> EventPredecessorOptions {
    let key = serde_json::json!([
        project.content_baseline(),
        project.root,
        format!("{:?}", project.authoring_diagnostics()),
        editor.baseline,
        editor.path,
        editor.original,
        editor.draft.id,
        editor.draft.period,
        editor.draft.predecessors,
    ])
    .to_string();
    if let Some((cached_key, options)) = &editor.temporal_cache {
        if cached_key == &key {
            return options.clone();
        }
    }
    let options = project.event_predecessor_options(
        &editor.path,
        editor.original.as_deref(),
        &editor.draft,
        &editor.baseline,
    );
    editor.temporal_cache = Some((key, options.clone()));
    options
}

pub(super) fn can_apply(options: &EventPredecessorOptions) -> bool {
    options.blocked_reason.is_none()
        && !options
            .entries
            .iter()
            .any(|entry| entry.selected && entry.rejection.is_some())
}

pub(super) fn application_status(ui: &mut egui::Ui, options: &EventPredecessorOptions) {
    if let Some(reason) = &options.blocked_reason {
        ui.colored_label(ERROR(), format!("无法应用：{reason}"));
    } else {
        let invalid = options
            .entries
            .iter()
            .filter(|entry| entry.selected && entry.rejection.is_some())
            .count();
        if invalid > 0 {
            ui.colored_label(ERROR(), format!("无法应用：{invalid} 条已选前驱需处理"));
            ui.label("草稿关系均已保留。恢复所属时段，或明确取消不合法的前驱。");
        }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    project: &Project,
    editor: &mut EventEditor,
    periods: &[PeriodInfo],
    stale: bool,
) {
    ui.separator();
    ui.label(egui::RichText::new("世界时间约束").strong());
    ui.label(theme::muted("所属时段"));
    egui::ComboBox::from_id_salt("event-period")
        .width(ui.available_width() - 20.0)
        .height(220.0)
        .selected_text(editor.draft.period.as_deref().unwrap_or("未分配时段"))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut editor.draft.period, None, "未分配时段");
            for period in periods {
                ui.selectable_value(
                    &mut editor.draft.period,
                    Some(period.id.clone()),
                    format!("{} · {}", period.display, period.id),
                )
                .on_hover_text(format!(
                    "时段 ID：{}\n时间根：{}\n来源：{}:{}",
                    period.id,
                    period.root.as_deref().unwrap_or("未知"),
                    period.file,
                    period.line
                ));
            }
        });
    // 更换时段只修改 period；前驱始终保留到作者明确取消。
    let options = options(project, editor);
    ui.label(theme::muted(match options.order_scope {
        TemporalOrderScope::RootPeriod => "合法范围：同一时间根（允许跨直接时段）",
        TemporalOrderScope::DirectPeriod => "合法范围：同一直接时段（当前语言版本）",
    }));
    ui.add(
        egui::Label::new(format!(
            "直接时段：{} · 时间根：{}",
            options.period.as_deref().unwrap_or("未分配"),
            options.root.as_deref().unwrap_or("未知")
        ))
        .wrap(),
    );
    ui.label(theme::muted("明确晚于以下事件；未连接事件不要求固定顺序。"));
    ui.add(
        egui::TextEdit::singleline(&mut editor.predecessor_query)
            .id_salt("event-predecessor-filter")
            .hint_text("筛选候选：名称 / ID / 时段 / 根 / 来源")
            .desired_width(f32::INFINITY),
    );
    let query = editor.predecessor_query.trim().to_lowercase();
    let selected_valid: Vec<_> = options
        .entries
        .iter()
        .filter(|entry| entry.selected && entry.rejection.is_none())
        .collect();
    let selected_invalid: Vec<_> = options
        .entries
        .iter()
        .filter(|entry| entry.selected && entry.rejection.is_some())
        .collect();
    let candidates: Vec<_> = options
        .entries
        .iter()
        .filter(|entry| !entry.selected && entry.rejection.is_none())
        .collect();
    let matches: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|entry| searchable(entry).to_lowercase().contains(&query))
        .collect();
    ui.label(format!(
        "已选 {} · 需处理 {} · 候选匹配 {}/{}",
        selected_valid.len(),
        selected_invalid.len(),
        matches.len(),
        candidates.len()
    ));
    if options.blocked_reason.is_some() {
        ui.label("工程当前不可写，候选不能新增；已有输入仍可查看和整理。");
    } else if stale {
        ui.label("旧基线只读：不能新增前驱；已选关系和输入仍保留。");
    }
    egui::ScrollArea::vertical()
        .id_salt(("event-predecessors", &editor.draft.id))
        .max_height(250.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (title, entries) in [
                ("已选 · 需处理", &selected_invalid),
                ("已选 · 合法", &selected_valid),
                ("可选候选", &matches),
            ] {
                ui.label(egui::RichText::new(format!("{title}（{}）", entries.len())).strong());
                if entries.is_empty() {
                    ui.label(theme::muted("无"));
                }
                for entry in entries {
                    predecessor_row(
                        ui,
                        editor,
                        entry,
                        options.blocked_reason.is_none() && !stale,
                        project,
                    );
                }
            }
        });
    ui.label(theme::muted(
        "筛选不隐藏已选关系。应用时仍须通过全工程校验，包括已有后继。",
    ));
    options_cache_after_edit(project, editor);
    ui.separator();
}

fn options_cache_after_edit(project: &Project, editor: &mut EventEditor) {
    let _ = options(project, editor);
}

fn searchable(entry: &EventPredecessorOption) -> String {
    format!(
        "{} {} {} {} {}",
        entry.display,
        entry.id,
        entry.period.as_deref().unwrap_or(""),
        entry.root.as_deref().unwrap_or(""),
        entry.file.as_deref().unwrap_or("")
    )
}

fn predecessor_row(
    ui: &mut egui::Ui,
    editor: &mut EventEditor,
    entry: &EventPredecessorOption,
    writable: bool,
    project: &Project,
) {
    ui.push_id(&entry.id, |ui| {
        let mut selected = entry.selected;
        let identity = format!("{} ({})", entry.display, entry.id);
        let response = ui
            .scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.add_enabled(
                    writable || selected,
                    egui::Checkbox::new(&mut selected, &identity),
                )
                .on_hover_text(&identity)
            })
            .inner;
        if response.changed() {
            if selected {
                editor.draft.predecessors.push(entry.id.clone());
            } else {
                editor.draft.predecessors.retain(|id| id != &entry.id);
            }
        }
        ui.add(egui::Label::new(theme::muted(format!("ID：{}", entry.id))).truncate())
            .on_hover_text(&entry.id);
        ui.label(theme::muted(format!(
            "时段：{} · 根：{}",
            entry.period.as_deref().unwrap_or("未分配"),
            entry.root.as_deref().unwrap_or("未知")
        )));
        let file = entry.file.as_deref().map(std::path::Path::new);
        let source = file.map(|path| path.strip_prefix(&project.root).unwrap_or(path));
        let source = format!(
            "来源：{}:{}",
            source
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "未知".into()),
            entry
                .line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "?".into())
        );
        ui.add(egui::Label::new(theme::muted(&source)).truncate())
            .on_hover_text(source);
        if let Some(reason) = &entry.rejection {
            ui.colored_label(ERROR(), format!("无法应用：{reason}"));
        }
        ui.add_space(6.0);
    });
}
