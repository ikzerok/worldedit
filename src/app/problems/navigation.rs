use crate::app::{Tab, WorldeditApp};
use worldline_core::problems::ProblemPrecision;

impl WorldeditApp {
    pub(in crate::app) fn problems_shortcuts(&mut self, ctx: &egui::Context) {
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return;
        }
        let top = self.command_palette.focus_stack.last().map(|entry| entry.0);
        if top.is_some_and(|kind| kind != "problems") {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let previous =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F8));
            let next =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F8));
            if previous || next {
                self.open_problems(ctx);
                self.step_problem(ctx, previous, true);
                return;
            }
        }
        if !self.personal.settings.diagnostics
            || !ctx.memory(|memory| memory.has_focus(egui::Id::new("problems-list")))
        {
            return;
        }
        let previous =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let next =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        if previous || next {
            self.step_problem(ctx, previous, false);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            super::view::consume_activation(ctx, egui::Key::Enter);
            self.locate_problem(ctx, None);
        }
    }

    pub(in crate::app) fn step_problem(
        &mut self,
        ctx: &egui::Context,
        previous: bool,
        locate: bool,
    ) {
        let origin = locate.then(|| self.author_location(Some(ctx)));
        let Some(page) = &self.problems.page else {
            return;
        };
        let current = self
            .problems
            .selected
            .as_ref()
            .and_then(|id| page.entries.iter().position(|entry| &entry.id == id));
        let len = page.entries.len();
        if len == 0 {
            return;
        }
        let index = if let Some(index) = current {
            if previous && index == 0 && !self.problems.previous_pages.is_empty() {
                self.problem_page(true);
                self.problems
                    .page
                    .as_ref()
                    .map_or(0, |page| page.entries.len().saturating_sub(1))
            } else if !previous && index + 1 == len && page.next_cursor.is_some() {
                self.problem_page(false);
                0
            } else if previous {
                index.saturating_sub(1)
            } else {
                (index + 1).min(len - 1)
            }
        } else if previous {
            len - 1
        } else {
            0
        };
        if let Some(id) = self
            .problems
            .page
            .as_ref()
            .and_then(|page| page.entries.get(index))
            .map(|entry| entry.id.clone())
        {
            self.problems.select(id);
            if locate {
                self.locate_problem_from(ctx, None, origin);
            }
        }
    }

    pub(super) fn problem_page(&mut self, previous: bool) {
        if previous {
            let Some(cursor) = self.problems.previous_pages.pop() else {
                return;
            };
            self.problems.cursor = cursor;
        } else {
            let Some(cursor) = self
                .problems
                .page
                .as_ref()
                .and_then(|page| page.next_cursor.clone())
            else {
                return;
            };
            self.problems
                .previous_pages
                .push(self.problems.cursor.clone());
            self.problems.cursor = Some(cursor);
        }
        self.problems.refresh_query();
    }

    pub(super) fn locate_problem(&mut self, ctx: &egui::Context, related: Option<usize>) {
        self.locate_problem_from(ctx, related, None);
    }

    fn locate_problem_from(
        &mut self,
        ctx: &egui::Context,
        related: Option<usize>,
        origin: Option<crate::app::personal::Location>,
    ) {
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            self.problems.notice = Some("输入法草稿尚未提交，未离开当前位置".into());
            return;
        }
        if self.problems.stale(self.version) || self.problems.error.is_some() {
            self.problems.notice =
                Some("报告尚未在当前来源范围完成重检，请刷新后定位；当前位置与草稿已保留".into());
            return;
        }
        let Some(report) = self.problems.report.clone() else {
            return;
        };
        let Some(id) = self.problems.selected.clone() else {
            return;
        };
        match self.project.problem_location(&report, &id, related) {
            Err(error) => self.problems.error = Some(error.to_string()),
            Ok(location) => {
                let source_location = location.clone();
                let Some(relative) = &location.path else {
                    return;
                };
                if location.precision == ProblemPrecision::Unavailable {
                    self.problems.notice =
                        Some(location.reason.unwrap_or_else(|| "来源位置不可用".into()));
                    return;
                }
                let path = self.project.root.join(relative);
                let position = origin.unwrap_or_else(|| self.author_location(Some(ctx)));
                if location.precision == ProblemPrecision::Span {
                    let Some(range) = location.byte_range else {
                        return;
                    };
                    let source = self.project.document(&path).ok().map(str::to_owned);
                    let Some(source) = source else {
                        self.problems.notice = Some("来源当前无法读取，未沿用旧选区".into());
                        return;
                    };
                    if let Err(error) = self.project.verify_source_navigation(&path, &source) {
                        self.problems.reject_source_navigation(error);
                        return;
                    }
                    crate::app::search::request_diagnostic_selection(
                        ctx,
                        path.clone(),
                        source,
                        range.start..range.end,
                    );
                } else {
                    crate::app::search::clear_pending_selection(ctx);
                    if let Some((id, _)) = self.source_position_document(&path) {
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
                            if let Some(range) = state.cursor.char_range() {
                                state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(
                                        range.primary,
                                    )));
                                state.store(ctx, id);
                            }
                        }
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                    self.problems.notice =
                        Some("已打开问题文档；core只提供文档级位置，未伪造行列或选区".into());
                }
                self.remember_author_location(position);
                self.personal.restore_source = false;
                self.remember_problem_source(&report, &id, related, path.clone(), source_location);
                self.active_file = path;
                self.tab = Tab::Edit;
                self.jump = None;
                self.problems.focus_list = false;
                self.problems.narrow_detail = false;
            }
        }
    }
}
