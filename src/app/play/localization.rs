//! Locale 是真实会话输入；对照只呈现同一次求值，不再执行故事。
use crate::app::{Tab, WorldeditApp};
use crate::draft_rehearsal_worker::DisplayOutput;
use worldline_core::{
    localization::{
        LocalizationPresentationPolicy, LocalizationPresentationRequest,
        LocalizationPresentationSnapshot,
    },
    project::Project,
};
use worldline_runtime::{LocalizedPresentation, ReplayTrace, RuntimeLocalizationIdentity};

#[derive(Default)]
pub(in crate::app) struct LocaleUi {
    pub enabled: bool,
    pub locale: String,
    pub fallback: bool,
    pub parallel: bool,
}
impl LocaleUi {
    pub fn request(&self) -> Option<LocalizationPresentationRequest> {
        self.enabled.then(|| LocalizationPresentationRequest {
            schema_version: 1,
            target_locale: self.locale.trim().into(),
            policy: if self.fallback {
                LocalizationPresentationPolicy::SourceFallback
            } else {
                LocalizationPresentationPolicy::Strict
            },
        })
    }
}

pub(super) fn controls(
    ui: &mut egui::Ui,
    locale: &mut LocaleUi,
    suggested: &str,
    keyboard: &super::keyboard::PlayKeyboard,
    host: super::keyboard::SettingsHost,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label("体验语言");
        keyboard.setting_in(ui, host, "locale-source", |ui| {
            ui.selectable_value(&mut locale.enabled, false, "源文")
        });
        if keyboard
            .setting_in(ui, host, "locale-translation", |ui| {
                ui.selectable_value(&mut locale.enabled, true, "译文")
            })
            .clicked()
            && locale.locale.is_empty()
        {
            locale.locale = suggested.into();
        }
        if locale.enabled {
            keyboard.setting_in(ui, host, "locale-input", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut locale.locale)
                        .id_salt("ordinary-play-locale")
                        .desired_width(100.0)
                        .hint_text("zh-Hant"),
                )
            });
            let policy_label = "允许有标记的源文回退";
            let policy_text = egui::WidgetText::from(policy_label).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::FontSelection::Default,
            );
            let policy_width =
                (policy_text.size().x + ui.spacing().icon_width + ui.spacing().icon_spacing)
                    .max(ui.spacing().interact_size.y);
            // The semantic child starts with the remaining row; wrap its whole checkbox first.
            if ui.available_size_before_wrap().x < policy_width {
                ui.end_row();
            }
            keyboard.setting_in(ui, host, "locale-fallback", |ui| {
                ui.checkbox(&mut locale.fallback, policy_label)
            });
        }
    });
    if locale.enabled {
        ui.small("语言或策略变更仅用于下一次明确开始；现有会话保留原快照");
    }
}

pub(in crate::app::play) fn prepare_trace(
    project: &Project,
    trace: &ReplayTrace,
) -> Result<Option<LocalizationPresentationSnapshot>, String> {
    let Some(identity) = &trace.presentation else {
        return Ok(None);
    };
    let snapshot = project
        .prepare_localization_presentation(&identity.request)
        .map_err(|e| e.to_string())?;
    if snapshot.presentation_digest() != identity.presentation_digest {
        return Err("路径所用译文已变化；不能用新译文冒充旧记录，请重新录制这条路径".into());
    }
    Ok(Some(snapshot))
}

pub(in crate::app::play) fn identity_label(
    ui: &mut egui::Ui,
    identity: Option<&RuntimeLocalizationIdentity>,
    parallel: &mut bool,
) {
    identity_label_inner(ui, identity, parallel, None);
}

pub(in crate::app::play) fn identity_label_with_settings(
    ui: &mut egui::Ui,
    identity: Option<&RuntimeLocalizationIdentity>,
    parallel: &mut bool,
    keyboard: &super::keyboard::PlayKeyboard,
) {
    identity_label_inner(ui, identity, parallel, Some(keyboard));
}

fn identity_label_inner(
    ui: &mut egui::Ui,
    identity: Option<&RuntimeLocalizationIdentity>,
    parallel: &mut bool,
    keyboard: Option<&super::keyboard::PlayKeyboard>,
) {
    if let Some(identity) = identity {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("实际语言 · {}", identity.request.target_locale));
            ui.label(match identity.request.policy {
                LocalizationPresentationPolicy::Strict => "严格译文",
                LocalizationPresentationPolicy::SourceFallback => "允许逐项源文回退",
            });
            if let Some(keyboard) = keyboard {
                keyboard.setting(ui, "locale-parallel", |ui| {
                    ui.checkbox(parallel, "源译对照")
                });
            } else {
                ui.checkbox(parallel, "源译对照");
            }
        });
        ui.small("人物名、禁用理由及动作日志仍使用源文");
    }
}

pub(super) enum Navigation {
    Entry {
        presentation: LocalizedPresentation,
        translation: bool,
    },
    Read(worldline_core::TargetRef),
}

pub(in crate::app::play) fn item(
    ui: &mut egui::Ui,
    content: &str,
    links: &[worldline_core::navigation::RenderedLink],
    localization: Option<&LocalizedPresentation>,
    parallel: bool,
) -> Option<Navigation> {
    let Some(presentation) = localization else {
        return linked_text(ui, content, links).map(Navigation::Read);
    };
    let mut navigation = None;
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            let label = crate::app::localization_ui::status_text(presentation.status);
            ui.label(
                if presentation.status
                    == worldline_core::localization::LocalizationStatus::Translated
                {
                    format!("译文 · {label}")
                } else {
                    format!("源文回退 · {label}")
                },
            );
            if ui.small_button("查看此源文").clicked() {
                navigation = Some(Navigation::Entry {
                    presentation: presentation.clone(),
                    translation: false,
                });
            }
            if presentation.id.is_some()
                && presentation.status
                    != worldline_core::localization::LocalizationStatus::DuplicateId
                && ui.small_button("修正此译文").clicked()
            {
                navigation = Some(Navigation::Entry {
                    presentation: presentation.clone(),
                    translation: true,
                });
            }
        });
        if parallel && ui.available_width() >= 640.0 {
            ui.columns(2, |columns| {
                columns[0].small("源文 · 同次求值");
                if let Some(target) = linked_text(
                    &mut columns[0],
                    &presentation.source_content,
                    &presentation.source_links,
                ) {
                    navigation = Some(Navigation::Read(target));
                }
                columns[1].small("实际显示");
                if let Some(target) = linked_text(&mut columns[1], content, links) {
                    navigation = Some(Navigation::Read(target));
                }
            });
        } else {
            if let Some(target) = linked_text(ui, content, links) {
                navigation = Some(Navigation::Read(target));
            }
            if parallel {
                ui.small("源文 · 同次求值");
                if let Some(target) =
                    linked_text(ui, &presentation.source_content, &presentation.source_links)
                {
                    navigation = Some(Navigation::Read(target));
                }
            }
        }
    });
    navigation
}

pub(in crate::app::play) fn outputs(
    ui: &mut egui::Ui,
    outputs: &[DisplayOutput],
    parallel: bool,
) -> Option<Navigation> {
    let mut navigation = None;
    let (content, links) = transcript(outputs, false);
    if parallel {
        let (source, source_links) = transcript(outputs, true);
        if ui.available_width() >= 640.0 {
            ui.columns(2, |columns| {
                columns[0].small("源文 · 同次求值");
                if let Some(target) = linked_text(&mut columns[0], &source, &source_links) {
                    navigation = Some(Navigation::Read(target));
                }
                columns[1].small("实际显示");
                if let Some(target) = linked_text(&mut columns[1], &content, &links) {
                    navigation = Some(Navigation::Read(target));
                }
            });
        } else {
            ui.small("实际显示");
            if let Some(target) = linked_text(ui, &content, &links) {
                navigation = Some(Navigation::Read(target));
            }
            ui.small("源文 · 同次求值");
            if let Some(target) = linked_text(ui, &source, &source_links) {
                navigation = Some(Navigation::Read(target));
            }
        }
    } else if let Some(target) = linked_text(ui, &content, &links) {
        navigation = Some(Navigation::Read(target));
    }
    let fallbacks = outputs
        .iter()
        .filter(|output| {
            output.localization.as_ref().is_some_and(|p| {
                p.status != worldline_core::localization::LocalizationStatus::Translated
            })
        })
        .count();
    if fallbacks > 0 {
        ui.colored_label(
            crate::theme::WARNING(),
            format!("本次输出中 {fallbacks} 项使用源文回退；展开下方逐项核对原因"),
        );
    }
    egui::CollapsingHeader::new("逐项来源、状态与修正")
        .default_open(fallbacks > 0)
        .show(ui, |ui| {
            for (index, output) in outputs.iter().enumerate() {
                ui.push_id(("localized-output", index), |ui| {
                    if let Some(speaker) = &output.speaker {
                        ui.strong(speaker);
                    }
                    if let Some(request) = item(
                        ui,
                        &output.content,
                        &output.links,
                        output.localization.as_ref(),
                        parallel,
                    ) {
                        navigation = Some(request);
                    }
                });
            }
        });
    navigation
}

fn transcript(
    outputs: &[DisplayOutput],
    source: bool,
) -> (String, Vec<worldline_core::navigation::RenderedLink>) {
    let mut text = String::new();
    let mut links = Vec::new();
    for output in outputs {
        if output.new_line && !text.is_empty() {
            text.push('\n');
        }
        if let Some(speaker) = &output.speaker {
            text.push_str(speaker);
            text.push('：');
        }
        let (content, spans) = if source {
            output
                .localization
                .as_ref()
                .map(|p| (&p.source_content, &p.source_links))
                .unwrap_or((&output.content, &output.links))
        } else {
            (&output.content, &output.links)
        };
        let offset = text.len();
        text.push_str(content);
        links.extend(spans.iter().cloned().map(|mut link| {
            link.start += offset;
            link.end += offset;
            link
        }));
    }
    (text, links)
}

impl WorldeditApp {
    pub(in crate::app::play) fn open_localized_item(
        &mut self,
        ctx: &egui::Context,
        navigation: Navigation,
        locale: &str,
        draft: bool,
    ) {
        let guard = if draft {
            if self.verify_rehearsal_localization_navigation() {
                Ok(())
            } else {
                Err("草稿或工作区已变化；旧试演输出保留，请重新核对后定位".into())
            }
        } else {
            self.play
                .as_ref()
                .ok_or_else(|| "试玩已经关闭".to_owned())
                .and_then(|p| self.play_source_navigation_guard(&p.scope))
        };
        if let Err(error) = guard {
            self.message = Some(error);
            return;
        }
        let (metadata, translation) = match navigation {
            Navigation::Read(target) => {
                self.open_reading(target);
                return;
            }
            Navigation::Entry {
                presentation,
                translation,
            } => (presentation, translation),
        };
        if !draft {
            if let Err(error) = self.verify_play_localized_item(&metadata, locale) {
                self.message = Some(error);
                return;
            }
        }
        if translation {
            self.localization_ui
                .open_translation(locale, metadata.id.as_deref());
            self.localization_ui
                .expect_runtime_revision(metadata.source_revision);
            self.tab = Tab::Localization;
        } else if draft {
            self.return_rehearsal_localization_source(ctx, &metadata.source);
        } else {
            let result = self
                .play_localized_source_hit(&metadata)
                .and_then(|hit| self.go_author_source_position(ctx, &hit, true));
            match result {
                Ok(()) => {
                    self.message =
                        Some("已定位本次源文的完整语句或选择头；Alt+Left 返回同一试玩".into());
                }
                Err(error) => self.message = Some(error),
            }
        }
        self.play_keyboard.cancel();
    }

    fn verify_play_localized_item(
        &self,
        metadata: &LocalizedPresentation,
        locale: &str,
    ) -> Result<(), String> {
        let play = self.play.as_ref().ok_or("当前没有可定位的试玩")?;
        let story = play.story.as_ref().ok_or("当前没有真实运行会话")?;
        if story
            .presentation_identity()
            .is_none_or(|identity| identity.request.target_locale != locale)
        {
            return Err("此来源的语言与实际运行会话不一致；旧输出已保留".into());
        }
        let observed = play
            .localized_outputs
            .iter()
            .filter_map(|output| output.localization.as_ref())
            .chain(
                story
                    .choice_presentations()
                    .iter()
                    .filter_map(|choice| choice.localization.as_ref()),
            )
            .any(|actual| {
                actual.source == metadata.source
                    && actual.id == metadata.id
                    && actual.source_revision == metadata.source_revision
                    && actual.source_baseline == metadata.source_baseline
            });
        if !observed {
            return Err("来源不属于本次已显示输出或当前选择；请重新展开实际内容".into());
        }
        Ok(())
    }

    fn play_localized_source_hit(
        &self,
        metadata: &LocalizedPresentation,
    ) -> Result<worldline_core::search_replace::SearchMatch, String> {
        let play = self.play.as_ref().ok_or("当前没有可定位的试玩")?;
        let snapshot = self.snapshot.as_ref().ok_or("当前编译来源不可用")?;
        let hit = worldline_core::localization::localization_source_hit(
            &snapshot.result,
            &self.project.root,
            &metadata.source,
            false,
        )?;
        let source = self.project.document(&hit.path)?;
        if !play.scope.source_matches(&hit.path, source) {
            return Err("本地化来源已不是实际运行稿；旧输出已保留".into());
        }
        self.project.verify_source_navigation(&hit.path, source)?;
        Ok(hit)
    }
}

fn linked_text(
    ui: &mut egui::Ui,
    text: &str,
    links: &[worldline_core::navigation::RenderedLink],
) -> Option<worldline_core::TargetRef> {
    let mut selected = None;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let normal = egui::TextFormat {
        font_id: font.clone(),
        color: ui.visuals().text_color(),
        ..Default::default()
    };
    let accent = ui.visuals().hyperlink_color;
    let linked = egui::TextFormat {
        font_id: font,
        color: accent,
        underline: egui::Stroke::new(1.0_f32, accent),
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    let mut offset = 0;
    let mut valid_links = Vec::new();
    // One galley preserves runtime newlines/glue across UTF-8 link spans, including wrapped links.
    // The spans come from core/runtime; no source or token parsing happens in this renderer.
    for link in links {
        let (Some(prefix), Some(label)) =
            (text.get(offset..link.start), text.get(link.start..link.end))
        else {
            continue;
        };
        job.append(prefix, 0.0, normal.clone());
        job.append(label, 0.0, linked.clone());
        valid_links.push(link);
        offset = link.end;
    }
    if let Some(tail) = text.get(offset..) {
        job.append(tail, 0.0, normal);
    }
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let response = ui.add(
        egui::Label::new(galley.clone())
            .wrap()
            .sense(egui::Sense::CLICK),
    );
    if let Some(position) = response.hover_pos() {
        let local = position - response.rect.min;
        let mut byte = 0;
        'rows: for row in &galley.rows {
            for glyph in &row.glyphs {
                let rectangle = glyph.logical_rect().translate(row.pos.to_vec2());
                if rectangle.contains(egui::Pos2::new(local.x, local.y)) {
                    if let Some(link) = valid_links
                        .iter()
                        .find(|link| (link.start..link.end).contains(&byte))
                    {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if response.clicked() {
                            selected = Some(link.target.clone());
                        }
                    }
                    break 'rows;
                }
                byte += glyph.chr.len_utf8();
            }
            if row.ends_with_newline {
                byte += 1;
            }
        }
    }
    // Individual native links remain keyboard/screen-reader reachable without splitting prose.
    if !valid_links.is_empty() {
        ui.menu_button("文中链接", |ui| {
            for (index, link) in valid_links.iter().enumerate() {
                let label = text.get(link.start..link.end).unwrap_or_default();
                let label = if label.is_empty() {
                    format!("{}:{}", link.target.kind, link.target.id)
                } else {
                    label.to_owned()
                };
                let response = ui.push_id(index, |ui| ui.link(label)).inner;
                if response.gained_focus() {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    selected = Some(link.target.clone());
                    ui.close();
                }
            }
        });
    }
    selected
}

#[cfg(test)]
mod tests;
