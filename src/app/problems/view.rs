use crate::theme;
use worldline_core::{
    problems::{ProblemCoverageState, ProblemDomain, ProblemLocation, ProblemPrecision},
    Severity,
};

pub(super) fn domain_label(domain: ProblemDomain) -> &'static str {
    match domain {
        ProblemDomain::Content => "语言内容",
        ProblemDomain::Workspace => "工程注册",
        ProblemDomain::Maps => "地图",
        ProblemDomain::GraphViews => "关系视图",
        ProblemDomain::Presets => "视图预设",
        ProblemDomain::Comments => "审阅批注",
        ProblemDomain::Proposals => "修改提案",
        ProblemDomain::Templates => "工程模板",
        ProblemDomain::SavedQueries => "保存查询",
        ProblemDomain::Manuscripts => "书稿",
        ProblemDomain::ReaderProfiles => "读者配置",
        ProblemDomain::Localizations => "本地化",
    }
}
pub(super) fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "● 错误",
        Severity::Warning => "▲ 提醒",
        Severity::Hint => "◇ 提示",
    }
}
pub(super) fn severity_color(severity: Severity) -> egui::Color32 {
    match severity {
        Severity::Error => theme::ERROR(),
        Severity::Warning => theme::GOLD(),
        Severity::Hint => theme::BLUE(),
    }
}
pub(super) fn coverage_label(state: ProblemCoverageState) -> &'static str {
    match state {
        ProblemCoverageState::Checked => "已检查",
        ProblemCoverageState::Partial => "部分检查",
        ProblemCoverageState::Unavailable => "不可读取",
        ProblemCoverageState::NotApplicable => "不适用",
    }
}
pub(super) fn location_label(location: &ProblemLocation) -> String {
    let path = location.path.as_deref().unwrap_or("来源不可用");
    match location.precision {
        ProblemPrecision::Span => location
            .span
            .map(|s| format!("{path}:{}:{}", s.line, s.column))
            .unwrap_or_else(|| path.into()),
        ProblemPrecision::Document => format!("{path} · 仅文档位置"),
        ProblemPrecision::Unavailable => format!("{path} · 位置不可用"),
    }
}

pub(super) fn precision_label(location: &ProblemLocation) -> &'static str {
    match location.precision {
        ProblemPrecision::Span => location
            .context
            .as_ref()
            .filter(|context| context.version == 1)
            .map_or("原文范围（旧报告）", |context| {
                role_label(context.role)
            }),
        ProblemPrecision::Document => "仅文档位置",
        ProblemPrecision::Unavailable => "位置不可用",
    }
}

pub(super) fn role_label(role: worldline_core::problems::ProblemSourceRole) -> &'static str {
    use worldline_core::problems::ProblemSourceRole;
    match role {
        ProblemSourceRole::Target => "具体目标",
        ProblemSourceRole::Expression => "完整表达式",
        ProblemSourceRole::Statement => "语句上下文",
        ProblemSourceRole::Declaration => "声明上下文",
        ProblemSourceRole::Document => "文档上下文",
        ProblemSourceRole::Unavailable => "来源不可用",
    }
}

pub(super) fn reading_job(
    text: &str,
    settings: &crate::app::personal::Settings,
    monospace: bool,
) -> egui::text::LayoutJob {
    let font = if monospace {
        egui::FontId::monospace(settings.body_size)
    } else {
        egui::FontId::proportional(settings.body_size)
    };
    let mut job = egui::text::LayoutJob::simple(text.into(), font, theme::TEXT(), f32::INFINITY);
    for section in &mut job.sections {
        section.format.line_height = Some(settings.body_size * settings.line_spacing);
    }
    job
}

pub(super) fn reading_label(
    ui: &mut egui::Ui,
    text: &str,
    settings: &crate::app::personal::Settings,
    monospace: bool,
) {
    ui.add(egui::Label::new(reading_job(text, settings, monospace)).wrap());
}

pub(super) fn location_ui(
    ui: &mut egui::Ui,
    label: &str,
    scope: egui::Id,
    location: &ProblemLocation,
    current: bool,
    settings: &crate::app::personal::Settings,
) -> bool {
    ui.strong(label);
    let text = location_label(location);
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(&text).wrap());
        if focus_action(detail_button(ui, scope.with("copy"), "复制位置", true)) {
            ui.ctx().copy_text(text.clone());
        }
    });
    if let Some(reason) = &location.reason {
        reading_label(ui, reason, settings, false);
    }
    let enabled =
        current && location.precision != ProblemPrecision::Unavailable && location.path.is_some();
    let response = detail_button(
        ui,
        scope.with("locate"),
        if location.precision == ProblemPrecision::Document {
            "打开文档（无精确选区）"
        } else {
            "定位来源"
        },
        enabled,
    );
    let clicked = focus_action(response);
    super::excerpt::source_excerpt(ui, scope, location, settings);
    clicked
}

/// 两个各自限一行的文本层，不能把含换行的整张Button设为Truncate（那会只留下摘要）。
pub(super) fn problem_row(
    ui: &mut egui::Ui,
    problem: &worldline_core::problems::ProblemEntry,
    selected: bool,
    font: f32,
    height: f32,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("problem-row", &problem.id)),
        egui::Sense::click() & !egui::Sense::focusable_noninteractive(),
    );
    let background = if selected {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 3., background);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(0., 4.),
                egui::vec2(3., (rect.height() - 8.).max(0.)),
            ),
            1.,
            theme::ACCENT(),
        );
    }
    let source = format!(
        "{} · {}",
        location_label(&problem.primary),
        domain_label(problem.domain)
    );
    let title = format!(
        "{} · {}",
        severity_label(problem.severity),
        crate::visual::truncated(&problem.message, 90)
    );
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("problem-text", &problem.id))
            .max_rect(rect.shrink2(egui::vec2(6., 3.)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    row.set_clip_rect(rect.intersect(ui.clip_rect()));
    row.spacing_mut().item_spacing.y = 0.;
    row.add(
        egui::Label::new(
            egui::RichText::new(&title)
                .size(font)
                .color(severity_color(problem.severity)),
        )
        .truncate()
        .selectable(false),
    );
    row.add(
        egui::Label::new(
            egui::RichText::new(&source)
                .size((font * 0.85).max(12.))
                .color(theme::MUTED()),
        )
        .truncate()
        .selectable(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            format!("{title}; {source}"),
        )
    });
    response.on_hover_text(format!(
        "{}\n{} · {}",
        problem.message,
        location_label(&problem.primary),
        problem.code
    ))
}

/// 只在焦点进入时滚入，保留作者之后的手动阅读滚动。
pub(super) fn focus_action(response: egui::Response) -> bool {
    if response.gained_focus() {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    action_clicked(response)
}

/// 键盘激活只属于这次工具动作，不能再投递给本帧随后出现的新来源TextEdit。
pub(super) fn action_clicked(response: egui::Response) -> bool {
    let clicked = response.clicked();
    if clicked && !response.clicked_by(egui::PointerButton::Primary) {
        let (enter, space) = response.ctx.input(|input| {
            (
                input.key_pressed(egui::Key::Enter),
                input.key_pressed(egui::Key::Space),
            )
        });
        if enter {
            consume_activation(&response.ctx, egui::Key::Enter);
        }
        if space {
            consume_activation(&response.ctx, egui::Key::Space);
        }
    }
    clicked
}

/// 消费已确认的动作键和该键对应文本，保留普通文本与IME事件。
/// 调用方可先consume_key；因此不能依赖已经被消费的key_pressed状态。
pub(super) fn consume_activation(ctx: &egui::Context, activation: egui::Key) {
    if !matches!(activation, egui::Key::Enter | egui::Key::Space) {
        return;
    }
    ctx.input_mut(|input| {
        input.events.retain(|event| match event {
            egui::Event::Key {
                key, pressed: true, ..
            } => *key != activation,
            egui::Event::Text(text) => {
                !(activation == egui::Key::Enter && matches!(text.as_str(), "\n" | "\r")
                    || activation == egui::Key::Space && text == " ")
            }
            _ => true,
        })
    });
}

/// 动态段落折行不得改变键盘动作的身份；外观复用Button，交互绑定稳定问题/来源ID。
pub(super) fn detail_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let painted = ui.add(egui::Button::new(label).small().sense(egui::Sense::hover()));
        let response = ui.interact(painted.rect, id, egui::Sense::click());
        if response.has_focus() || response.hovered() {
            ui.painter().rect_stroke(
                response.rect,
                3.,
                egui::Stroke::new(
                    if response.has_focus() { 1.5_f32 } else { 1_f32 },
                    theme::ACCENT(),
                ),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        response
    })
    .inner
}
