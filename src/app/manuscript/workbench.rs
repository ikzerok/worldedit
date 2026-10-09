use super::*;
use crate::theme;
use worldline_core::manuscript::ManuscriptQueryDraft;

impl super::super::WorldeditApp {
    pub(in crate::app) fn manuscript_raw_input_hook(
        &mut self,
        ctx: &egui::Context,
        raw: &mut egui::RawInput,
    ) {
        // 非模态窗口可仍打开，但输入归属只由真实 TextEdit 焦点决定。
        let available = self.tab == super::super::Tab::Manuscript;
        self.manuscript
            .navigation
            .input
            .filter_raw(ctx, raw, available);
        self.manuscript
            .writing_view
            .filter_raw_input(ctx, raw, available);
    }

    pub(in crate::app) fn manuscript_tab(&mut self, ctx: &egui::Context) {
        let _navigation_composition = self.manuscript.navigation.input.begin(ctx);
        let _composition = self.manuscript.writing_view.begin_input(ctx);
        self.manuscript_tab_content(ctx);
        self.draw_manuscript_world_links(ctx);
    }

    fn manuscript_tab_content(&mut self, ctx: &egui::Context) {
        if self.manuscript.creating_new {
            self.manuscript_creation_tab(ctx);
            return;
        }
        self.manuscript_review_shortcut(ctx);
        let baseline = self.project.content_baseline();
        if self
            .manuscript
            .books
            .values()
            .any(|book| !book.changed && book.baseline != baseline)
            || self
                .manuscript
                .writing_buffers
                .values()
                .any(|buffer| !buffer.is_changed() && buffer.baseline() != baseline)
        {
            let receiver_book = self
                .manuscript
                .writing_view
                .composition_receiver(ctx)
                .and_then(|owner| {
                    self.manuscript
                        .books
                        .iter()
                        .find(|(_, local)| editing::owns_receiver(&self.project.root, local, owner))
                        .map(|(id, _)| id.clone())
                });
            self.manuscript
                .rebase_clean_preserving(&self.project, receiver_book.as_deref());
        }
        let applied_snapshot = match self.manuscript.query_cache.applied(&self.project) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.colored_label(theme::ERROR(), format!("书稿导航不可用：{error}"));
                    ui.label("全部输入仍保留；请修复工程后重试。");
                    self.manuscript_orphaned_drafts(ui);
                });
                return;
            }
        };
        let indices = applied_snapshot.indices();
        let mut session = self.manuscript.pending_session.take();
        let mut restore_failed = false;
        if let Some(saved) = session.as_mut() {
            match self.manuscript.validate_session(&self.project, saved) {
                Err(error) => {
                    self.message = Some(error);
                    restore_failed = true;
                    saved.cursor = None;
                    saved.anchor = None;
                    saved.restore_offsets = Some(false);
                }
                Ok(false) => {
                    saved.cursor = None;
                    saved.restore_offsets = Some(false);
                }
                Ok(true) => {}
            }
        }
        if let Some(session) = &session {
            if session
                .manuscript_id
                .as_ref()
                .is_some_and(|id| indices.contains_key(id))
            {
                self.manuscript
                    .selected_book
                    .clone_from(&session.manuscript_id);
            } else if session.manuscript_id.is_some() {
                self.message =
                    Some("上次书稿 ID 已失效；请明确选择书稿与章节，未按同名标题替换身份".into());
            }
        }
        if self
            .manuscript
            .selected_book
            .as_ref()
            .is_none_or(|id| !indices.contains_key(id))
        {
            self.manuscript.selected_book = indices.keys().next().cloned();
        }
        let Some(book_id) = self.manuscript.selected_book.clone() else {
            if !applied_snapshot.complete() {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("书稿范围无法确认");
                    ui.label(
                        "当前注册或源码存在问题；这不表示作品没有章节。原始文档与输入仍保留。",
                    );
                    let diagnostics = applied_snapshot.diagnostics();
                    let offset = &mut self.manuscript.navigation.diagnostic_offset;
                    *offset = (*offset).min(diagnostics.len().saturating_sub(1) / 8 * 8);
                    ui.horizontal_wrapped(|ui| {
                        if crate::theme::add_enabled(ui, *offset > 0, egui::Button::new("前8项"))
                            .clicked()
                        {
                            *offset = offset.saturating_sub(8);
                        }
                        if crate::theme::add_enabled(
                            ui,
                            *offset + 8 < diagnostics.len(),
                            egui::Button::new("后8项"),
                        )
                        .clicked()
                        {
                            *offset += 8;
                        }
                    });
                    for diagnostic in diagnostics.iter().skip(*offset).take(8) {
                        ui.colored_label(
                            theme::ERROR(),
                            format!("{}：{}", diagnostic.code, diagnostic.message),
                        );
                    }
                    self.manuscript_orphaned_drafts(ui);
                });
            } else {
                self.manuscript_start_tab(ctx);
            }
            return;
        };
        let Some(index) = indices.get(&book_id) else {
            return;
        };
        self.manuscript
            .books
            .entry(book_id.clone())
            .or_insert_with(|| {
                let draft = ManuscriptDraft::from_index(index);
                let selected_entry = draft
                    .entries
                    .iter()
                    .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                    .or_else(|| draft.entries.first())
                    .map(|entry| entry.id.clone());
                LocalBook {
                    original: draft.clone(),
                    draft,
                    baseline: self.project.content_baseline(),
                    revision: Revision::default(),
                    selected_entry,
                    changed: false,
                    collapsed: HashSet::new(),
                }
            });
        let mut local = self.manuscript.books.remove(&book_id).unwrap();
        if let Some(mut session) = session {
            if session.manuscript_id.as_deref() == Some(&book_id) {
                if let Some(navigation) = session.navigation.take() {
                    self.manuscript.layout = navigation.layout;
                    local.collapsed = navigation.collapsed.iter().cloned().collect();
                    self.manuscript.navigation.restore(navigation);
                }
                if let Some(id) = session
                    .selected_id
                    .clone()
                    .filter(|id| local.draft.entries.iter().any(|entry| &entry.id == id))
                {
                    if session.restore_offsets.unwrap_or(false) {
                        if let Some(anchor) = &session.anchor {
                            self.manuscript.chapter_sources.insert(
                                (book_id.clone(), id.clone()),
                                (anchor.target.clone(), anchor.path.clone()),
                            );
                        }
                    }
                    local.selected_entry = Some(id);
                    self.manuscript.pending_scroll = Some(
                        if session.restore_offsets.unwrap_or(false) && session.scroll_y.is_finite()
                        {
                            session.scroll_y.clamp(0.0, 1_000_000.0)
                        } else {
                            0.0
                        },
                    );
                    self.manuscript.pending_review_scroll = Some(
                        if session.restore_offsets.unwrap_or(false)
                            || self
                                .manuscript
                                .preview_cache
                                .can_restore_delivery_view(&session)
                        {
                            session
                                .review_scroll_y
                                .filter(|offset| offset.is_finite())
                                .unwrap_or(0.0)
                                .clamp(0.0, 1_000_000.0)
                        } else {
                            0.0
                        },
                    );
                    self.manuscript.review_page_offset = session.review_page_offset.unwrap_or(0);
                    self.manuscript.reader_open = session.preview_open.unwrap_or(true);
                    self.manuscript.reader_whole_book = session.preview_whole_book.unwrap_or(false);
                    self.manuscript.preview_cache.scoped = session.preview_scoped.unwrap_or(false);
                    self.manuscript
                        .preview_cache
                        .restore_delivery_page(session.review_page_offset.unwrap_or(0));
                    self.manuscript.narrow_preview = session.preview_tab.unwrap_or(false);
                    self.manuscript.review_focus = self.manuscript.narrow_preview;
                    self.manuscript.writing_view.restore_mode(session.mode);
                    self.manuscript.writing_view.restore_cursor(session.cursor);
                } else {
                    local.selected_entry = None;
                    if session.selected_id.is_some() {
                        self.message = Some("上次章节 ID 已失效，请明确选择章节".into());
                    }
                }
            }
        }
        if restore_failed {
            local.selected_entry = None;
        }
        let drafts: Vec<_> = local
            .changed
            .then(|| ManuscriptQueryDraft {
                expected_baseline: local.baseline.clone(),
                draft: local.draft.clone(),
            })
            .into_iter()
            .collect();
        let navigation_snapshot = if drafts.is_empty()
            && self
                .manuscript
                .writing_buffers
                .values()
                .all(|buffer| !buffer.is_changed())
        {
            Ok(applied_snapshot.clone())
        } else {
            self.manuscript.query_cache.current(
                &self.project,
                self.manuscript.writing_buffers.values(),
                &drafts,
            )
        };
        self.manuscript
            .preview_cache
            .set_query_snapshot(navigation_snapshot.clone().ok());
        if local.selected_entry.as_deref().is_some_and(|id| {
            navigation_snapshot
                .as_ref()
                .is_ok_and(|snapshot| snapshot.entry_is_ambiguous(&book_id, id))
        }) {
            local.selected_entry = None;
            self.message = Some("原章节身份重复，未打开任意同 ID 替身；正文草稿仍保留。".into());
        }
        let preview = navigation_snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.indices().get(&book_id))
            .unwrap_or(index);
        let error = navigation_snapshot.as_ref().err();
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.clone())
            .unwrap_or_default();
        let input_locked = self.review_input_blocker(ctx).is_some();
        // 自己的IME必须仍由原筛选TextEdit接收；其他编排/正文动作继续用完整锁。
        let navigation_locked = input_locked && !self.manuscript.navigation.input.blocked();
        let compact_workspace = layout::compact_workspace(ctx);
        let focus_layout = self.manuscript_focus_layout() || compact_workspace;
        let focus_style = self.focus_style();
        let mut create_book = false;
        let mut create_chapter = false;
        let mut apply_book = false;
        let mut discard_book = false;
        let mut body_action = None;
        let mut rehearse_draft = false;
        let can_rehearse = !input_locked
            && self
                .manuscript
                .writing_buffers
                .values()
                .any(|buffer| buffer.is_changed());
        let mut panel = theme::panel();
        if compact_workspace {
            panel.inner_margin.top = 4;
            panel.inner_margin.bottom = 4;
        }
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            if focus_layout {
                crate::theme::add_enabled_ui(ui, !navigation_locked, |ui| {
                    let single_preview = focus_style
                        || !layout::parallel_review(
                            ui.available_width(),
                            self.personal.appearance().body_size,
                        );
                    (create_chapter, rehearse_draft) = super::focus_controls::draw(
                        ui,
                        &mut self.manuscript,
                        &mut local,
                        index.title.as_deref().unwrap_or(&book_id),
                        index.read_only,
                        single_preview,
                        &navigation_snapshot,
                        &self.project.root,
                        can_rehearse,
                    );
                });
            }
            if !focus_layout || self.manuscript.focus_management {
                let mut management = |ui: &mut egui::Ui| {
                    crate::theme::add_enabled_ui(ui, !input_locked, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let title = index.title.as_deref().unwrap_or(&book_id);
                            ui.add_sized(
                                [
                                    ui.available_width().min(180.0),
                                    ui.spacing().interact_size.y,
                                ],
                                egui::Label::new(egui::RichText::new(title).heading()).truncate(),
                            )
                            .on_hover_text(title);
                            egui::ComboBox::from_id_salt("manuscript-book-picker")
                                .selected_text(index.title.as_deref().unwrap_or(&book_id))
                                .show_ui(ui, |ui| {
                                    for (id, book) in indices {
                                        ui.selectable_value(
                                            &mut self.manuscript.selected_book,
                                            Some(id.clone()),
                                            format!(
                                                "{} · {id}",
                                                book.title.as_deref().unwrap_or(id)
                                            ),
                                        );
                                    }
                                });
                            if ui.button("新建书稿").clicked() {
                                create_book = true;
                            }
                            ui.selectable_value(
                                &mut self.manuscript.layout,
                                Layout::Tree,
                                "章节树",
                            );
                            ui.selectable_value(&mut self.manuscript.layout, Layout::List, "列表");
                            ui.selectable_value(&mut self.manuscript.layout, Layout::Cards, "卡片");
                            ui.checkbox(&mut self.manuscript.reader_open, "阅读预览");
                        });
                        ui.label(theme::muted(
                            "书稿只决定阅读顺序，独立于世界时间与事件控制流；正文编辑另行应用。",
                        ));
                        if !index.diagnostics.is_empty() {
                            ui.colored_label(
                                theme::WARNING(),
                                format!(
                                    "编排有 {} 项诊断；在章节导航的快照问题中逐页查看。",
                                    index.diagnostics.len()
                                ),
                            );
                        }
                        if let Some(error) = &error {
                            ui.colored_label(theme::ERROR(), format!("书稿草稿：{error}"));
                        }
                        ui.horizontal_wrapped(|ui| {
                            if !focus_layout
                                && crate::theme::add_enabled(
                                    ui,
                                    !index.read_only,
                                    theme::primary("新建章节"),
                                )
                                .clicked()
                            {
                                create_chapter = true;
                            }
                            if crate::theme::add_enabled(
                                ui,
                                !index.read_only,
                                egui::Button::new("插入分节"),
                            )
                            .clicked()
                            {
                                insert_entry(&mut local, ManuscriptEntryKind::Section);
                            }
                            if crate::theme::add_enabled(
                                ui,
                                !index.read_only,
                                egui::Button::new("插入章节"),
                            )
                            .clicked()
                            {
                                insert_entry(&mut local, ManuscriptEntryKind::Chapter);
                            }
                            if crate::theme::add_enabled(
                                ui,
                                !index.read_only && local.changed && error.is_none(),
                                egui::Button::new("应用书稿"),
                            )
                            .clicked()
                            {
                                apply_book = true;
                            }
                            if crate::theme::add_enabled(
                                ui,
                                local.changed,
                                egui::Button::new("恢复书稿草稿"),
                            )
                            .on_hover_text("丢弃未应用编排，保留正文文件草稿")
                            .clicked()
                            {
                                discard_book = true;
                            }
                            if !focus_layout {
                                // Share the existing action row: a fixed rehearsal row
                                // can push the first prose line outside a short viewport.
                                ui.menu_button("试演", |ui| {
                                    rehearse_draft = crate::theme::add_enabled(
                                        ui,
                                        can_rehearse,
                                        egui::Button::new("试演当前正文草稿…"),
                                    )
                                    .on_hover_text(
                                        "明确核对范围后独立运行；不应用、保存或替换普通试玩",
                                    )
                                    .clicked();
                                    if rehearse_draft {
                                        ui.close();
                                    }
                                });
                            }
                            if local.changed {
                                ui.label(theme::muted("编排尚未应用"));
                            }
                        });
                        ui.separator();
                    });
                };
                if compact_workspace {
                    egui::ScrollArea::vertical()
                        .id_salt("compact-manuscript-management")
                        .max_height((ui.available_height() * 0.35).min(160.0))
                        .min_scrolled_height(0.0)
                        .show(ui, management);
                } else {
                    management(ui);
                }
            }
            if !focus_layout && ui.available_width() >= 760.0 {
                egui::SidePanel::left("manuscript-outline")
                    .default_width(240.0)
                    .width_range(200.0..=360.0)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        crate::theme::add_enabled_ui(ui, !navigation_locked, |ui| {
                            outline::draw(
                                ui,
                                self.manuscript.layout,
                                &mut local,
                                &navigation_snapshot,
                                &mut self.manuscript.navigation,
                                &self.project.root,
                                false,
                            );
                        });
                    });
            } else if !focus_layout {
                egui::CollapsingHeader::new("章节")
                    .id_salt(("manuscript-narrow-outline", &book_id))
                    .show(ui, |ui| {
                        crate::theme::add_enabled_ui(ui, !navigation_locked, |ui| {
                            outline::draw(
                                ui,
                                self.manuscript.layout,
                                &mut local,
                                &navigation_snapshot,
                                &mut self.manuscript.navigation,
                                &self.project.root,
                                false,
                            );
                        });
                    });
            }
            body_action =
                self.draw_manuscript_content(ui, &book_id, &mut local, &catalog, index, preview);
        });
        if std::mem::take(&mut self.manuscript.navigation.enter_editor) {
            self.manuscript.narrow_preview = false;
            self.manuscript.writing_view.focus_existing_editor();
        }
        if discard_book {
            local.draft = ManuscriptDraft::from_index(index);
            local.original = local.draft.clone();
            local.baseline = self.project.content_baseline();
            local.changed = false;
        }
        local.changed = local.draft != local.original;
        if create_chapter {
            self.begin_chapter_creation(Some(&local));
        }
        if create_book {
            self.begin_chapter_creation(None);
        }
        self.manuscript.books.insert(book_id.clone(), local);
        if let Some((path, action)) = body_action {
            if action.world_link {
                self.begin_manuscript_world_links(ctx);
            }
            if action.comment {
                self.comment_current_selection(ctx);
            }
            if let Some(error) = action.error {
                self.io_error = Some(error);
            }
            if action.discard {
                self.discard_manuscript_body(&path);
            } else if action.apply {
                self.apply_manuscript_body(&path, action.source_mode);
            }
        }
        if apply_book {
            self.apply_manuscript_book(&book_id);
        }
        self.finish_review_navigation(ctx);
        self.finish_manuscript_delivery_actions(ctx);
        if rehearse_draft {
            self.request_draft_rehearsal(ctx);
        }
    }
}

fn insert_entry(local: &mut LocalBook, kind: ManuscriptEntryKind) {
    let parent_id = editing::selected_section(local);
    let id = unique_id(
        &local.draft.entries,
        if kind == ManuscriptEntryKind::Section {
            "section"
        } else {
            "chapter"
        },
    );
    local.draft.entries.push(ManuscriptEntryDraft {
        id: id.clone(),
        kind,
        parent_id,
        title: if kind == ManuscriptEntryKind::Section {
            "新分节"
        } else {
            "新章节"
        }
        .into(),
        summary: None,
        pov: None,
        status: Some("draft".into()),
        goal: None,
        target_ref: None,
    });
    local.selected_entry = Some(id);
    local.changed = true;
}
