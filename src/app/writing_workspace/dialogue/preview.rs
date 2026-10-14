use super::*;

pub(super) fn migration(ui: &mut egui::Ui, plan: &DialogueEditPlan, typography: Typography) {
    let Some(migration) = &plan.migration else {
        return;
    };
    ui.strong(format!(
        "全稿解释变化 · 语言 {} → {}",
        migration.current_language.as_str(),
        migration.target_language.as_str()
    ));
    ui.label(format!(
        "required_features：{} → {}",
        migration.required_features_before.join("、"),
        migration.required_features_after.join("、")
    ));
    for note in &migration.compatibility_notes {
        ui.add(egui::Label::new(theme::muted(note)).wrap());
    }
    if migration.fingerprint_comparison_reliable {
        ui.label(format!(
            "迁移运行指纹 {:016x} → {:016x}",
            migration.runtime_fingerprint_before, migration.runtime_fingerprint_after
        ));
    } else {
        ui.colored_label(
            theme::WARNING(),
            "全文有编译诊断，迁移运行指纹不能用来保证兼容性。",
        );
    }
    if plan.fingerprint_comparison_reliable {
        ui.label(format!(
            "本次完整操作指纹 {:016x} → {:016x}",
            plan.runtime_fingerprint_before, plan.runtime_fingerprint_after
        ));
    } else {
        ui.colored_label(theme::WARNING(), "此受阻计划不能比较完整操作运行指纹。");
    }
    preview_keyboard::collapsing(
        ui,
        format!("关键字重新解释 · {}处", migration.keyword_changes.len()),
        |ui| {
            let range = page_range(
                ui,
                migration.keyword_changes.len(),
                "dialogue-migration-keywords",
                &plan.plan_digest,
            );
            for change in &migration.keyword_changes[range] {
                ui.label(format!(
                    "{}:{} · {} → {}",
                    change.file, change.line, change.before_kind, change.after_kind
                ));
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&change.source)
                            .font(theme::source_font(typography.source_size)),
                    )
                    .wrap(),
                );
            }
        },
    );
    preview_keyboard::collapsing(
        ui,
        format!(
            "全文候选诊断 · {}项（新增{}项）",
            migration.diagnostics_after.len(),
            migration.new_diagnostics.len()
        ),
        |ui| {
            let range = page_range(
                ui,
                migration.diagnostics_after.len(),
                "dialogue-migration-diagnostics",
                &plan.plan_digest,
            );
            for diagnostic in &migration.diagnostics_after[range] {
                ui.colored_label(
                    if diagnostic.severity == worldline_core::Severity::Error {
                        theme::ERROR()
                    } else {
                        theme::WARNING()
                    },
                    format!(
                        "{} · {}:{} · {}",
                        diagnostic.code, diagnostic.file, diagnostic.span.line, diagnostic.message
                    ),
                );
            }
        },
    );
    if !migration.can_apply {
        ui.colored_label(
            theme::ERROR(),
            "全稿迁移候选未通过检查，不能确认应用；原稿保持不变。",
        );
    }
    preview_keyboard::collapsing(
        ui,
        format!("本次一次提交的完整文件 · {}个", plan.changes.len()),
        |ui| {
            let range = page_range(
                ui,
                plan.changes.len(),
                "dialogue-migration-files",
                &plan.plan_digest,
            );
            for change in &plan.changes[range] {
                ui.push_id(&change.path, |ui| {
                    ui.strong(change.path.to_string_lossy());
                    ui.label(if change.includes_unapplied_draft {
                        "包含此文件完整未应用输入"
                    } else {
                        "仅所列已验证变化"
                    });
                    preview_keyboard::collapsing(ui, "完整变更前字节", |ui| {
                        if let Some(before) = &change.before {
                            text(
                                ui,
                                before,
                                (&plan.plan_digest, "before", &change.path),
                                typography,
                            );
                        } else {
                            ui.label("原来没有这个文件");
                        }
                    });
                    preview_keyboard::collapsing(ui, "完整变更后字节", |ui| {
                        text(
                            ui,
                            &change.after,
                            (&plan.plan_digest, "after", &change.path),
                            typography,
                        );
                    });
                });
            }
        },
    );
}
fn page_range(ui: &mut egui::Ui, total: usize, role: &str, digest: &str) -> std::ops::Range<usize> {
    let id = egui::Id::new((role, digest));
    let mut offset = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(total.saturating_sub(1) / 20 * 20);
    ui.horizontal_wrapped(|ui| {
        if preview_keyboard::add(ui, offset > 0, egui::Button::new("前20项")).clicked() {
            offset = offset.saturating_sub(20);
        }
        if preview_keyboard::add(ui, offset + 20 < total, egui::Button::new("后20项")).clicked() {
            offset += 20;
        }
        ui.label(format!(
            "{}–{} / {}",
            if total == 0 { 0 } else { offset + 1 },
            (offset + 20).min(total),
            total
        ));
    });
    ui.ctx().data_mut(|data| data.insert_temp(id, offset));
    offset..(offset + 20).min(total)
}
pub(super) fn text(
    ui: &mut egui::Ui,
    text: &str,
    salt: impl std::hash::Hash,
    typography: Typography,
) {
    const BYTES: usize = 16 * 1024;
    let id = egui::Id::new(salt);
    let count = text.len().div_ceil(BYTES).max(1);
    let mut page = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(count - 1);
    ui.horizontal_wrapped(|ui| {
        if preview_keyboard::add(ui, page > 0, egui::Button::new("前一段字节")).clicked() {
            page -= 1;
        }
        if preview_keyboard::add(ui, page + 1 < count, egui::Button::new("后一段字节")).clicked()
        {
            page += 1;
        }
        ui.label(format!(
            "第{} / {}段 · {} UTF-8字节",
            page + 1,
            count,
            text.len()
        ));
    });
    let boundary = |mut index: usize| {
        index = index.min(text.len());
        while !text.is_char_boundary(index) {
            index -= 1;
        }
        index
    };
    let mut segment = &text[boundary(page * BYTES)..boundary((page + 1) * BYTES)];
    ui.add(
        egui::TextEdit::multiline(&mut segment)
            .font(theme::source_font(typography.source_size))
            .interactive(false)
            .desired_width(f32::INFINITY),
    );
    ui.ctx().data_mut(|data| data.insert_temp(id, page));
}
