use super::creation::{CreationState, SourceChoice};
use super::*;
use crate::theme;

impl super::super::WorldeditApp {
    pub(super) fn manuscript_start_tab(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("manuscript-start")
                    .show(ui, |ui| {
                        ui.add_space((ui.clip_rect().height() * 0.08).clamp(20.0, 64.0));
                        theme::document_surface(ui, 760.0, |ui| {
                            ui.label(theme::muted("书稿 · 从这里开始"));
                            ui.add_space(theme::SPACE_MD);
                            ui.label(
                                egui::RichText::new("写下第一章")
                                    .font(theme::body_font(30.0))
                                    .strong(),
                            );
                            ui.add_space(theme::SPACE_LG);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new("给作品和章节起个名字，开始写正文。")
                                        .font(theme::body_font(18.0)),
                                )
                                .wrap(),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new("人物与地点资料，按需添加。")
                                        .font(theme::body_font(18.0)),
                                )
                                .wrap(),
                            );
                            ui.add_space(theme::SPACE_XL);
                            let unfinished = self
                                .manuscript
                                .creation
                                .as_ref()
                                .filter(|form| form.touched);
                            if let Some(form) = unfinished {
                                ui.label(egui::RichText::new(&form.chapter_title).strong());
                                ui.label(theme::muted("创建尚未应用，刚才的输入已保留"));
                            }
                            if ui
                                .add(theme::primary(if unfinished.is_some() {
                                    "继续创建首章"
                                } else {
                                    "开始写作"
                                }))
                                .clicked()
                            {
                                self.begin_chapter_creation(None);
                            }
                            ui.add_space(theme::SPACE_MD);
                            ui.label(theme::muted(
                                "填写标题与来源  →  核对创建计划  →  写下第一段",
                            ));
                            ui.add_space(theme::SPACE_XL);
                            ui.separator();
                            ui.add_space(theme::SPACE_LG);
                            ui.label(egui::RichText::new("已经有文字了？").strong());
                            ui.label(theme::muted(
                                "导入现有 Markdown，先核对来源、附件与转换计划。",
                            ));
                            if ui.button("导入 Markdown").clicked() {
                                self.markdown_import_wizard
                                    .get_or_insert_with(Default::default);
                            }
                            ui.add_space(theme::SPACE_LG);
                            ui.label(theme::muted("书稿组织阅读顺序，正文保留在原文件。"));
                            self.manuscript_orphaned_drafts(ui);
                        });
                    });
            });
    }

    pub(super) fn manuscript_creation_tab(&mut self, ctx: &egui::Context) {
        let mut form = self
            .manuscript
            .creation
            .take()
            .unwrap_or_else(|| CreationState::new(&self.project, None));
        let blocker = self.chapter_creation_blocker(ctx);
        let composing = self.review_input_blocker(ctx).is_some();
        let mut back = false;
        let mut clear = false;
        let mut preview = false;
        let mut apply = false;
        let mut import = false;
        if !composing
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            back = true;
        }
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("chapter-create-scroll")
                    .show(ui, |ui| {
                        theme::document_surface(ui, 760.0, |ui| {
                            theme::panel_header(
                                ui,
                                if form.reviewing {
                                    "核对新章节"
                                } else if form.existing_book.is_some() {
                                    "新建章节"
                                } else {
                                    "开始你的书稿"
                                },
                                if form.reviewing { "确认内容与文件，再开始写作" } else { "一章正文，开始一部作品" },
                            );
                            if !form.reviewing {
                            ui.label(theme::muted("1 · 标题与正文来源"));
                            ui.add_space(theme::SPACE_MD);
                            let changed = self.draw_chapter_creation_fields(ui, &mut form);
                            if changed {
                                form.invalidate();
                            }
                            if let Some(reason) = &blocker {
                                ui.colored_label(theme::WARNING(), reason);
                            }
                            if let Some((request, _)) = &form.preview {
                                if request.expected_baseline != self.project.content_baseline() {
                                    ui.colored_label(
                                        theme::WARNING(),
                                        "工程已变化，旧计划不能应用。输入已保留，请重新预览。",
                                    );
                                }
                            }
                            ui.add_space(theme::SPACE_LG);
                            ui.horizontal_wrapped(|ui| {
                                preview = crate::theme::add_enabled(ui, blocker.is_none(), theme::primary("预览创建计划"))
                                    .clicked();
                                back |= crate::theme::add_enabled(ui, !composing, egui::Button::new("返回，保留输入"))
                                    .clicked();
                                if crate::theme::add_enabled(ui,
                                        !composing && form.touched,
                                        egui::Button::new("清空创建输入"),
                                    )
                                    .clicked()
                                {
                                    form.clear_confirm = true;
                                }
                                import = crate::theme::add_enabled(ui, !composing, egui::Button::new("导入 Markdown"))
                                    .clicked();
                            });
                            if form.clear_confirm {
                                ui.label("将清空这份创建表单，不改变工程正文或已应用内容。");
                                ui.horizontal_wrapped(|ui| {
                                    clear = crate::theme::add_enabled(ui,
                                            !composing,
                                            egui::Button::new("确认清空创建输入"),
                                        )
                                        .clicked();
                                    if ui.button("保留创建输入").clicked() {
                                        form.clear_confirm = false;
                                    }
                                });
                            }
                            }
                            if let Some(error) = &form.error {
                                ui.colored_label(theme::ERROR(), error);
                            }
                            if form.reviewing {
                            if let Some((request, plan)) = &form.preview {
                                let book_title = form.existing_book.as_ref().and_then(|id| self.project.manuscript_indices().get(id).and_then(|book| book.title.clone())).or_else(|| form.existing_book.clone()).unwrap_or_default();
                                if request.expected_baseline != self.project.content_baseline() {
                                    ui.colored_label(theme::WARNING(), "工程已变化，旧计划不能应用。请选择修改计划后重新预览，输入会保留。");
                                }
                                if let Some(reason) = &blocker { ui.colored_label(theme::WARNING(), reason); }
                                ui.add_space(theme::SPACE_LG);
                                super::creation_plan::draw(ui, request, plan, &book_title);
                                apply = crate::theme::add_enabled(ui,
                                        blocker.is_none()
                                            && plan.can_apply
                                            && request.expected_baseline
                                                == self.project.content_baseline(),
                                        theme::primary("创建并写作"),
                                    )
                                    .clicked();
                                ui.horizontal_wrapped(|ui| {
                                    if crate::theme::add_enabled(ui, !composing, egui::Button::new("修改创建计划")).clicked() { form.reviewing = false; }
                                    back |= crate::theme::add_enabled(ui, !composing, egui::Button::new("返回，保留输入")).clicked();
                                });
                            }
                            }
                        });
                    });
            });
        if preview {
            self.preview_chapter_creation(&mut form);
        }
        if apply && blocker.is_none() && self.apply_chapter_creation(&mut form) {
            return;
        }
        if clear {
            form = CreationState::new(
                &self.project,
                form.existing_book
                    .as_ref()
                    .and_then(|id| self.manuscript.books.get(id)),
            );
        }
        if back {
            self.manuscript.creating_new = false;
            self.manuscript.creation_dismissed = true;
        }
        if import {
            self.markdown_import_wizard
                .get_or_insert_with(Default::default);
        }
        self.manuscript.creation = Some(form);
    }

    fn draw_chapter_creation_fields(&self, ui: &mut egui::Ui, form: &mut CreationState) -> bool {
        let mut changed = false;
        let books = self.project.manuscript_indices();
        if !books.is_empty() {
            let old = form.existing_book.clone();
            egui::ComboBox::from_id_salt("chapter-create-book")
                .selected_text(
                    form.existing_book
                        .as_ref()
                        .and_then(|id| books.get(id))
                        .and_then(|book| book.title.as_deref())
                        .unwrap_or("新书稿"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.existing_book, None, "新书稿");
                    for (id, book) in &books {
                        ui.selectable_value(
                            &mut form.existing_book,
                            Some(id.clone()),
                            book.title.as_deref().unwrap_or(id),
                        )
                        .on_hover_text(format!("manuscript:{id}"));
                    }
                });
            if old != form.existing_book {
                form.parent = None;
                form.after = None;
                changed = true;
            }
        }
        if form.existing_book.is_none() {
            ui.label(egui::RichText::new("书名").strong());
            let title = ui.add(
                egui::TextEdit::singleline(&mut form.book_title)
                    .id_salt("chapter-create-book-title")
                    .font(theme::body_font(22.0))
                    .hint_text("这部作品叫什么？")
                    .desired_width(f32::INFINITY),
            );
            super::super::writing_workspace::register_input(&title);
            changed |= title.changed();
            if form.focus_title {
                title.request_focus();
            }
        }
        ui.add_space(theme::SPACE_MD);
        ui.label(egui::RichText::new("章名").strong());
        let title = ui.add(
            egui::TextEdit::singleline(&mut form.chapter_title)
                .id_salt("chapter-create-chapter-title")
                .font(theme::body_font(20.0))
                .hint_text("这一章的标题")
                .desired_width(f32::INFINITY),
        );
        super::super::writing_workspace::register_input(&title);
        changed |= title.changed();
        if form.focus_title && form.existing_book.is_some() {
            title.request_focus();
        }
        form.focus_title = false;
        ui.add_space(theme::SPACE_LG);
        ui.label(egui::RichText::new("正文从哪里开始？").strong());
        ui.horizontal_wrapped(|ui| {
            changed |= ui
                .selectable_value(
                    &mut form.source_choice,
                    SourceChoice::NewEvent,
                    "新建空白正文",
                )
                .changed();
            changed |= ui
                .selectable_value(
                    &mut form.source_choice,
                    SourceChoice::Existing,
                    "引用已有正文",
                )
                .changed();
        });
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| &snapshot.result.analysis.catalog);
        if let Some(catalog) = catalog {
            if form.source_choice != SourceChoice::NewEvent
                && catalog.object(&TargetRef::new("event", "start")).is_some()
                && ui
                    .button("明确使用现有起点 start")
                    .on_hover_text("引用 event:start；保留它现在的内容与运行入口")
                    .clicked()
            {
                form.source_choice = SourceChoice::Existing;
                form.target = Some(TargetRef::new("event", "start"));
                changed = true;
            }
            if form.source_choice == SourceChoice::Existing {
                changed |= super::super::object_picker::object_picker(
                    ui,
                    "chapter-create-source",
                    "正文来源",
                    &mut form.target,
                    catalog,
                    &["event", "scene", "entity", "fragment"],
                );
            }
        }
        ui.label(theme::muted(match form.source_choice {
            SourceChoice::Unselected => "请选择一种来源；不会默认引用目录中的任何对象。",
            SourceChoice::Existing => "章节引用同一份正式内容；原有正文会完整保留。",
            SourceChoice::NewEvent => {
                "创建一份独立正文，保持当前语言版本；需要试玩时，可再安排它的运行路线。"
            }
        }));
        if form.source_choice == SourceChoice::NewEvent {
            ui.label(theme::muted(format!(
                "当前语言 {} · 创建正文不会升级语言",
                self.project.language_version()
            )));
        }
        egui::CollapsingHeader::new("高级：稳定身份与存放位置")
            .id_salt("chapter-create-advanced")
            .show(ui, |ui| {
                ui.label(theme::muted(
                    "建议值可修改；最终身份和全部文件会在创建计划中明示。",
                ));
                if form.existing_book.is_none() {
                    field(ui, "书稿 ID", &mut form.book_id, &mut changed);
                }
                field(ui, "章节 ID", &mut form.chapter_id, &mut changed);
                if form.source_choice == SourceChoice::NewEvent {
                    field(ui, "事件 ID", &mut form.event_id, &mut changed);
                    changed |= ui.checkbox(&mut form.new_file, "新建独立源文件").changed();
                    field(ui, "相对工作区路径", &mut form.source_path, &mut changed);
                    field(ui, "事件线", &mut form.storyline, &mut changed);
                }
                if let Some(book) = form.existing_book.as_ref().and_then(|id| books.get(id)) {
                    let old = form.parent.clone();
                    egui::ComboBox::from_id_salt("chapter-create-parent")
                        .selected_text(form.parent.as_deref().unwrap_or("书稿根目录"))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.parent, None, "书稿根目录");
                            for entry in book
                                .entries
                                .iter()
                                .filter(|entry| entry.kind == ManuscriptEntryKind::Section)
                            {
                                ui.selectable_value(
                                    &mut form.parent,
                                    Some(entry.id.clone()),
                                    &entry.title,
                                )
                                .on_hover_text(&entry.id);
                            }
                        });
                    if old != form.parent {
                        form.after = None;
                        changed = true;
                    }
                    let old = form.after.clone();
                    egui::ComboBox::from_id_salt("chapter-create-after")
                        .selected_text(
                            form.after
                                .as_deref()
                                .map(|id| format!("排在 {id} 后"))
                                .unwrap_or_else(|| "追加到末尾".into()),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.after, None, "追加到末尾");
                            for entry in book
                                .entries
                                .iter()
                                .filter(|entry| entry.parent_id == form.parent)
                            {
                                ui.selectable_value(
                                    &mut form.after,
                                    Some(entry.id.clone()),
                                    format!("排在 {} 后", entry.title),
                                )
                                .on_hover_text(&entry.id);
                            }
                        });
                    changed |= old != form.after;
                }
            });
        changed
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, changed: &mut bool) {
    ui.label(label);
    let response = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    super::super::writing_workspace::register_input(&response);
    *changed |= response.changed();
}
