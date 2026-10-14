use super::*;

pub(super) struct Keyboard {
    pub flow: Flow,
    tick: u32,
    pub response: Option<egui::Response>,
    pub gained: Vec<egui::WidgetInfo>,
    pub trace: Vec<String>,
    pub outer_clip: Option<Rect>,
    pub areas: Vec<(String, egui::Id, egui::Response)>,
    pub pass: u64,
}
impl Keyboard {
    pub fn new(lines: usize, large: bool, query_fixture: bool) -> Self {
        let mut flow = Flow::new(lines);
        // Fixture construction only. All tested filter/delivery changes below use
        // actual controls; no snapshot/artifact/confirmation/destination is injected.
        if large || query_fixture {
            let entry = flow.app.project.entry.clone();
            let mut source = flow.app.project.document(&entry).unwrap().to_owned();
            if large {
                let wide = "远航🦀UTF8_".repeat(100);
                let mut body = format!("BEGIN_WIDE_{wide}_END_WIDE");
                for n in 0..100 {
                    body.push_str(&format!(
                        "\\nBYTE_ROW_{n:03} 长字节完整核对。🦀长字节完整核对。"
                    ));
                }
                source = source.replace("Line 00", &body);
            }
            if query_fixture {
                let mut calls = String::new();
                for n in 0..21 {
                    calls.push_str(&format!("  call extra_{n:02}()\n"));
                    source.push_str(&format!("fragment extra_{n:02}()\n  return\n"));
                }
                source = source.replace(
                    "  say b \"OTHER_ROLE_SECRET\"",
                    &format!("{calls}  say b \"OTHER_ROLE_SECRET\""),
                );
                let path = flow.app.project.root.join(".world/book.json");
                let mut book: Value = serde_json::from_slice(
                    flow.app.project.authoring_document(&path).unwrap().bytes(),
                )
                .unwrap();
                for n in 0..21 {
                    book["entries"].as_array_mut().unwrap().push(json!({"id":format!("extra_{n:02}"),"kind":"chapter","title":format!("Extra {n:02}"),"target_ref":{"kind":"event","id":"start"}}));
                }
                flow.app
                    .project
                    .set_authoring_document(&path, serde_json::to_vec(&book).unwrap())
                    .unwrap();
                let mut locale = flow.locale();
                locale["entries"].as_object_mut().unwrap().remove("main_2");
                flow.set_locale(&locale);
            }
            flow.app.project.set_text(&entry, source).unwrap();
            flow.app.project.save().unwrap();
            flow.app.reset_views();
            flow.app.recompile();
            let navigation = flow
                .app
                .manuscript
                .plan_review_match(&TargetRef::new("event", "start"), &entry, &flow.app.project)
                .unwrap();
            flow.app
                .manuscript
                .apply_writing_match(navigation, &flow.app.project)
                .unwrap();
        }
        flow.app.manuscript.production = State::default();
        flow.app.open_production_script();
        flow.app.personal.settings.appearance.reduce_motion = true;
        let mut h = Self {
            flow,
            tick: 0,
            response: None,
            gained: vec![],
            trace: vec![],
            outer_clip: None,
            areas: vec![],
            pass: 0,
        };
        h.settle();
        h
    }
    pub fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.tick += 1;
        let modifiers = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Key { modifiers, .. }
                | Event::PointerButton { modifiers, .. }
                | Event::MouseWheel { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(Modifiers::NONE);
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            time: Some(f64::from(self.tick) / 60.0),
            focused: true,
            modifiers,
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.flow.app, &self.flow.ctx, &mut raw);
        let mut response = None;
        let mut areas = vec![];
        let mut pass = 0;
        let out = self.flow.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.flow.app, ctx, &mut eframe::Frame::_new_kittest());
            pass = ctx.cumulative_pass_nr();
            response = ctx
                .memory(|m| m.focused())
                .and_then(|id| ctx.read_response(id));
            areas = Self::scroll_states(ctx)
                .into_iter()
                .filter_map(|(name, id, _)| {
                    let area = id.with("area");
                    // Reject fallback from prev_pass: only actual current UI receivers.
                    ctx.viewport(|v| v.this_pass.widgets.get(area).is_some())
                        .then(|| ctx.read_response(area))
                        .flatten()
                        .map(|r| (name, id, r))
                })
                .collect();
        });
        self.pass = pass;
        self.response = response;
        self.areas = areas;
        if let Some((_, _, clip)) = texts(&out).into_iter().find(|(s, _, _)| s == "范围") {
            self.outer_clip = Some(clip);
        }
        for event in &out.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                self.gained.push(info.clone());
            }
        }
        out
    }
    pub fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    pub fn key(&mut self, key: Key, modifiers: Modifiers) -> egui::FullOutput {
        self.gained.clear();
        let mut commands = vec![];
        for pressed in [true, false] {
            let out = self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
            commands.extend(out.platform_output.commands);
        }
        let mut out = self.frame(vec![]);
        commands.append(&mut out.platform_output.commands);
        // Preserve real command delivery order while geometry remains the last pass.
        out.platform_output.commands = commands;
        out
    }
    pub fn type_value(&mut self, value: &str) {
        assert!(self
            .response
            .as_ref()
            .is_some_and(|r| egui::TextEdit::load_state(&self.flow.ctx, r.id).is_some()));
        self.key(Key::A, Modifiers::COMMAND);
        if value.is_empty() {
            // Empty Text events are ignored by TextEdit. Delete the actual selection.
            self.key(Key::Backspace, Modifiers::NONE);
        } else {
            self.frame(vec![Event::Text(value.into())]);
        }
        self.frame(vec![]);
    }
    pub fn offsets(&self) -> Vec<(String, egui::Id, egui::Vec2)> {
        Self::scroll_states(&self.flow.ctx)
    }
    fn scroll_states(ctx: &egui::Context) -> Vec<(String, egui::Id, egui::Vec2)> {
        // Read only: actual stable child-id construction in egui 0.32.3. A loaded
        // state must exist; there is no synthetic scroll state or offset write.
        let mut result = vec![];
        let mut id = egui::Id::new((egui::ViewportId::ROOT, "central_panel"));
        for _ in 0..5 {
            id = id.with(egui::Id::new("child"));
            for name in ["production-script-workbench", "production-artifact-bytes"] {
                let key = id.with(egui::Id::new(name));
                if let Some(s) = egui::scroll_area::State::load(ctx, key) {
                    result.push((name.into(), key, s.offset));
                }
            }
        }
        result
    }
    pub fn author_state(&self) -> Value {
        let app = &self.flow.app;
        let sources: BTreeMap<_, _> = app.project.sources().into_iter().collect();
        let buffers: BTreeMap<_, _> = app
            .manuscript
            .writing_buffers()
            .into_iter()
            .map(|b| (b.path().to_owned(), b.identity()))
            .collect();
        json!({"baseline":app.project.content_baseline(),"sources":sources,"buffers":buffers,
            "fields":app.manuscript.runtime_drafts(&app.project.root),"history":[app.history.len(),app.redo.len(),app.search_state.undo.len(),app.search_state.redo.len()],"version":app.version,"dirty":app.project.is_dirty()})
    }
    pub fn diagnostic(&self, out: &egui::FullOutput) -> String {
        format!("owner={:?} response={:?} input_focused={} popup={} offsets={:?} parent_paint_clip={:?} query={:?} page={} artifact_page={} options={:?} confirmed={} notice={} texts={:?} trace={:?}",
            self.flow.ctx.memory(|m| m.focused()), self.response.as_ref().map(|r| (r.id,r.rect,r.interact_rect,r.sense,r.has_focus())), self.flow.ctx.input(|i| i.focused), egui::Popup::is_any_open(&self.flow.ctx), self.offsets(), self.outer_clip, self.flow.app.manuscript.production.snapshot.as_ref().map(|s| s.key()), self.flow.app.manuscript.production.offset, self.flow.app.manuscript.production.artifact_page, self.flow.app.manuscript.production.artifact_options, self.flow.app.manuscript.production.confirmed, self.flow.notice(), texts(out), self.trace)
    }
    pub fn tab_to(&mut self, label: &str) -> egui::FullOutput {
        let mut last = self.frame(vec![]);
        for n in 0..360 {
            let mut out = self.key(Key::Tab, Modifiers::NONE);
            if let Some(r) = self.response.clone() {
                self.trace.push(format!(
                    "tab{n} target={label} id={:?} rect={:?} interact={:?} gained={:?}",
                    r.id, r.rect, r.interact_rect, self.gained
                ));
                let named = self
                    .gained
                    .iter()
                    .any(|i| i.label.as_deref() == Some(label));
                let drawn =
                    visible(&out, label).is_some_and(|(rect, _)| owns(&self.flow.ctx, &r, rect));
                if named || drawn {
                    if !r.interact_rect.contains_rect(r.rect)
                        && out.viewport_output[&egui::ViewportId::ROOT].repaint_delay
                            == Duration::ZERO
                    {
                        out = self.frame(vec![]);
                        assert_eq!(self.response.as_ref().map(|r| r.id), Some(r.id));
                    }
                    let current = self.response.as_ref().unwrap();
                    assert_eq!(
                        self.flow.ctx.memory(|m| m.focused()),
                        Some(current.id),
                        "same-pass control remains the actual owner"
                    );
                    assert!(
                        visible(&out, label).is_some_and(|(rect, clip)| owns(
                            &self.flow.ctx,
                            current,
                            rect
                        ) && clip
                            .contains_rect(current.rect)
                            && current.interact_rect.contains_rect(current.rect)),
                        "keyboard target is not wholly visible: {label}; {}",
                        self.diagnostic(&out)
                    );
                    assert!(
                        focus_paint(
                            &self.flow.ctx,
                            &out,
                            current.rect,
                            label == "生成当前稿台本"
                        ) || (self
                            .gained
                            .iter()
                            .any(|i| i.typ == egui::WidgetType::Checkbox
                                && i.label.as_deref() == Some(label))
                            && checkbox_focus_paint(&self.flow.ctx, &out, current)),
                        "keyboard target has no visible focus: {label}; {}",
                        self.diagnostic(&out)
                    );
                    return out;
                }
            }
            last = out;
        }
        panic!("Tab did not reach {label}; {}", self.diagnostic(&last));
    }
    pub fn enter(&mut self, label: &str) -> egui::FullOutput {
        self.tab_to(label);
        let out = self.key(Key::Enter, Modifiers::NONE);
        if label == "复制相同完整材料" {
            assert_eq!(
                out.platform_output
                    .commands
                    .iter()
                    .filter(|command| matches!(command, egui::OutputCommand::CopyText(_)))
                    .count(),
                1,
                "one real Enter emits exactly one complete material copy"
            );
        }
        out
    }
    pub fn tab_field(&mut self, hint: &str) {
        let mut last = self.frame(vec![]);
        for n in 0..360 {
            let mut out = self.key(Key::Tab, Modifiers::NONE);
            self.trace.push(format!(
                "field{n} target={hint} response={:?} gained={:?}",
                self.response
                    .as_ref()
                    .map(|r| (r.id, r.rect, r.interact_rect)),
                self.gained
            ));
            let search = hint == "完整范围查找"
                && self.response.as_ref().is_some_and(|r| {
                    egui::TextEdit::load_state(&self.flow.ctx, r.id).is_some()
                        && visible(&out, hint).is_some_and(|(label, clip)| {
                            label.right() <= r.rect.left()
                                && (label.center().y - r.rect.center().y).abs() < label.height()
                                && clip.contains_rect(r.rect)
                        })
                });
            if search
                || self
                    .gained
                    .iter()
                    .any(|i| i.hint_text.as_deref() == Some(hint))
            {
                let old = self.response.as_ref().expect("focused field").clone();
                if !old.interact_rect.contains_rect(old.rect)
                    && out.viewport_output[&egui::ViewportId::ROOT].repaint_delay == Duration::ZERO
                {
                    out = self.frame(vec![]);
                    assert_eq!(self.response.as_ref().map(|r| r.id), Some(old.id));
                }
                let r = self.response.as_ref().expect("focused field");
                assert!(egui::TextEdit::load_state(&self.flow.ctx, r.id).is_some());
                assert!(
                    r.has_focus() && r.interact_rect.contains_rect(r.rect),
                    "field is not fully visible: {hint}; {}",
                    self.diagnostic(&out)
                );
                assert!(
                    text_focus_paint(&self.flow.ctx, &out, r),
                    "field needs visible focus: {hint}; {}",
                    self.diagnostic(&out)
                );
                return;
            }
            last = out;
        }
        panic!("Tab did not reach field {hint}; {}", self.diagnostic(&last));
    }
    pub fn generate_keyboard(&mut self) {
        self.enter("生成当前稿台本");
        self.finish_job();
    }
    pub fn finish_job(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while self.flow.app.manuscript.production.job.is_some() {
            assert!(
                Instant::now() < deadline,
                "job did not finish: {}",
                self.flow.notice()
            );
            self.frame(vec![]);
            std::thread::sleep(Duration::from_millis(2));
        }
        self.settle();
    }
}
