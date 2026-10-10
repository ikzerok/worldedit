use super::*;
use worldline_core::manuscript::{
    ManuscriptBookDestination, ManuscriptChapterCreateRequest, ManuscriptChapterDraft,
    ManuscriptChapterSource,
};
use worldline_core::project::Project;

pub(super) struct Harness {
    pub ctx: egui::Context,
    pub app: WorldeditApp,
    pub size: Vec2,
    tick: u32,
    reading_domain: Option<(egui::Id, Rect)>,
    pub focused_response: Option<egui::Response>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
impl Harness {
    pub fn new(long: bool) -> Self {
        Self::fixture(long, false, None)
    }
    pub fn paged() -> Self {
        Self::fixture(false, true, None)
    }
    pub fn normal(statement: &str) -> Self {
        Self::normal_language(statement, "1.11")
    }
    pub fn normal_language(statement: &str, language: &str) -> Self {
        Self::fixture(false, false, Some((statement, language)))
    }
    fn fixture(long: bool, paged: bool, normal: Option<(&str, &str)>) -> Self {
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        static NEXT_ROOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "migration-keyboard-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        // Never reuse another concurrently allocated fixture, regardless of clock resolution.
        std::fs::create_dir(&root).expect("each fixture exclusively creates a fresh root");
        app.project = Project::new(&root);
        app.active_file = app.project.entry.clone();
        let mut source = "character traveler as \"旅人\"\nevent arrival\n  -> END\n".to_owned();
        if long {
            for n in 0..40 {
                source.push_str(&format!(
                    "// MIGRATION_LINE_{n:02} 完整只读来源字节，用逐行可见几何证明阅读范围。\n"
                ));
            }
        }
        if paged {
            for n in 0..21 {
                source.push_str(&format!("event unreachable_{n:02}\n  -> END\n"));
            }
            for n in 0..260 {
                source.push_str(&format!(
                    "// PAGE_ROW_{n:03} 完整只读来源字节。完整只读来源字节。完整只读来源字节。\n"
                ));
            }
            assert!(source.len() > 16 * 1024 && source.len() < 32 * 1024);
        }
        if let Some((statement, _)) = normal {
            source =
                format!("character traveler as \"旅人\"\nevent arrival\n{statement}  -> END\n");
        }
        app.project.set_text(&app.active_file, source).unwrap();
        let language = normal.map_or("1.9", |(_, language)| language);
        let manifest = r#"{"schema_version":1,"language_version":"1.9","required_features":[],"maps":{},"graph_views":{}}"#
            .replace("1.9", language).into_bytes();
        app.project
            .create_authoring_document(&root.join(".world/project.json"), manifest)
            .unwrap();
        app.project.save().unwrap();
        let mut revision = Revision::default();
        let request = ManuscriptChapterCreateRequest {
            schema_version: 1,
            expected_baseline: app.project.content_baseline(),
            expected_revision: revision,
            book: ManuscriptBookDestination::New {
                id: "book".into(),
                title: "键盘迁移书稿".into(),
            },
            chapter: ManuscriptChapterDraft {
                id: "chapter".into(),
                title: "arrival".into(),
                parent_section_id: None,
                after_sibling_id: None,
            },
            source: ManuscriptChapterSource::Existing {
                target: TargetRef::new("event", "arrival"),
            },
        };
        let plan = app
            .project
            .preview_manuscript_chapter_create(revision, &request)
            .unwrap();
        app.project
            .apply_manuscript_chapter_create(&mut revision, &request, &plan.plan_digest)
            .unwrap();
        app.reset_views();
        app.recompile();
        app.tab = Tab::Manuscript;
        app.personal.settings.appearance.reduce_motion = true;
        let mut h = Self {
            ctx,
            app,
            size: egui::vec2(1188.0, 848.0),
            tick: 0,
            reading_domain: None,
            focused_response: None,
        };
        h.settle();
        h
    }
    pub fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.tick += 1;
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } | Event::PointerButton { modifiers, .. } => {
                    Some(*modifiers)
                }
                _ => None,
            })
            .unwrap_or(Modifiers::NONE);
        let mut raw = egui::RawInput {
            modifiers,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(f64::from(self.tick) / 60.0),
            focused: true,
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let mut focused_response = None;
        let out = self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
            // end_pass swaps current/previous widget maps. Capture here, after the
            // real App has drawn all widgets, paired with this pass's output.
            // A real multi-pass run overwrites this with its final painted pass.
            focused_response = ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id));
        });
        self.focused_response = focused_response;
        out
    }
    pub fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    pub fn key(&mut self, key: Key, modifiers: Modifiers) -> egui::FullOutput {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.frame(vec![])
    }
    pub fn space(&mut self) {
        self.frame(vec![
            Event::Key {
                key: Key::Space,
                physical_key: Some(Key::Space),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(" ".into()),
        ]);
        self.frame(vec![Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }
    pub fn click(&mut self, label: &str) {
        let out = self.settle();
        let rect = visible_label(&out, label)
            .unwrap_or_else(|| panic!("missing fully visible {label}: {}", labels(&out)));
        let pos = rect.center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }
    pub fn toolbar(&mut self, label: &str) {
        let mut out = self.settle();
        if visible_label(&out, label).is_none() {
            if visible_label(&out, "正文工具").is_some() {
                self.click("正文工具");
                out = self.settle();
            }
            assert!(
                visible_label(&out, "对白工具").is_some(),
                "{}",
                labels(&out)
            );
            self.click("对白工具");
        }
        self.click(label);
    }
    pub fn begin_fields(&mut self) -> egui::Id {
        self.toolbar("插入正式台词");
        let buffer = self.app.manuscript.writing_buffers().pop().unwrap();
        let projection = self
            .app
            .project
            .project_dialogue_buffer(&buffer, &TargetRef::new("event", "arrival"))
            .unwrap();
        let anchor = &projection.anchors[0];
        let label = format!("第{}行 · {}", anchor.line, anchor.label);
        self.click("明确选择插入位置");
        self.click(&label);
        self.click("在此写正式台词");
        let owner = self
            .ctx
            .memory(|m| m.focused())
            .expect("new literal field naturally focused");
        assert!(egui::TextEdit::load_state(&self.ctx, owner).is_some());
        self.frame(vec![Event::Text(BODY.into())]);
        self.click("明确选择正式角色");
        self.click("旅人 · character:traveler");
        owner
    }
    pub fn preview(&mut self) -> egui::Id {
        let owner = self.begin_fields();
        self.click(ENABLE);
        self.click("预览语句变更");
        let out = self.settle();
        assert!(
            labels(&out).contains("正式语句变更预览"),
            "{}",
            labels(&out)
        );
        assert_eq!(self.app.project.language_version(), "1.9");
        owner
    }
    pub fn tab_to(&mut self, label: &str) -> egui::Response {
        self.tab_near(label, None)
    }
    pub fn tab_near(&mut self, label: &str, caption: Option<&str>) -> egui::Response {
        self.tab_excluding(label, caption, &std::collections::HashSet::new())
    }
    fn tab_excluding(
        &mut self,
        label: &str,
        caption: Option<&str>,
        excluded: &std::collections::HashSet<egui::Id>,
    ) -> egui::Response {
        for _ in 0..160 {
            let out = self.key(Key::Tab, Modifiers::NONE);
            let Some(id) = self.ctx.memory(|m| m.focused()) else {
                continue;
            };
            let Some(r) = self.focused_response.clone() else {
                continue;
            };
            assert_eq!(r.id, id, "paint-pass owner must match the final real focus");
            if excluded.contains(&id) {
                continue;
            }
            // egui ScrollArea is focusable Sense::drag; its rect encloses every child.
            // Only the exact local click-only widget can own this label's Tab stop.
            if !r.sense.senses_click() || r.sense.senses_drag() {
                continue;
            }
            let matching = texts(&out)
                .into_iter()
                .find(|(text, rect, _)| text == label && control_owns_label(&self.ctx, &r, *rect));
            if let Some((_, rect, clip)) = matching {
                if caption.is_some_and(|caption| {
                    !texts(&out).iter().any(|(text, near, _)| {
                        text == caption && (near.center().y - rect.center().y).abs() < rect.height()
                    })
                }) {
                    continue;
                }
                if label == READ || label == NORMAL_READ {
                    for (text, other, other_clip) in texts(&out) {
                        if [
                            "章节导航",
                            "整书 · manuscript:book",
                            "键盘迁移书稿",
                            "正文工具",
                            "写作",
                            "结构",
                            "源码",
                        ]
                        .contains(&text.as_str())
                            && other_clip.contains_rect(other)
                        {
                            assert!(!clip.intersects(other),"reading clip overlaps non-document control {text}: {clip:?} / {other:?}");
                        }
                    }
                    self.reading_domain = Some((r.id, clip));
                }
                assert!(
                    r.has_focus() && clip.contains_rect(rect),
                    "focused {label} not completely visible: {rect:?} / {clip:?}"
                );
                assert!(
                    has_control_focus_paint(&self.ctx, &out, r.rect),
                    "focused {label} needs a distinguishable local focus outline: {:?}",
                    r.rect
                );
                return r;
            }
        }
        panic!("real Tab never reached visible local control {label}");
    }
    pub fn tab_to_text(&mut self, owner: egui::Id, value: &str) {
        for _ in 0..160 {
            let out = self.key(Key::Tab, Modifiers::NONE);
            if self.ctx.memory(|memory| memory.focused()) != Some(owner) {
                continue;
            }
            let response = self
                .focused_response
                .clone()
                .expect("original field is drawn");
            assert_eq!(response.id, owner, "same-pass original TextEdit response");
            assert!(egui::TextEdit::load_state(&self.ctx, owner).is_some());
            let rect =
                visible_label(&out, value).expect("original text field must be fully revealed");
            assert!(response.has_focus() && response.rect.expand(1.0).contains_rect(rect));
            assert!(has_control_focus_paint(&self.ctx, &out, response.rect));
            return;
        }
        panic!("real Tab did not return to the exact original TextEdit owner");
    }
    pub fn expand_distinct_file_bytes(&mut self, count: usize) {
        let mut before = std::collections::HashSet::new();
        let mut after = std::collections::HashSet::new();
        for _ in 0..count {
            for (label, opened) in [
                ("完整变更前字节", &mut before),
                ("完整变更后字节", &mut after),
            ] {
                let response = self.tab_excluding(label, None, opened);
                assert!(
                    opened.insert(response.id),
                    "each real file section opens only once"
                );
                self.key(Key::Enter, Modifiers::NONE);
                assert!(
                    egui::collapsing_header::CollapsingState::load(&self.ctx, response.id)
                        .is_some_and(|state| state.is_open()),
                    "the actual file section must be open"
                );
            }
        }
        assert_eq!(before.len(), count);
        assert_eq!(after.len(), count);
    }
    pub fn enter(&mut self, label: &str) {
        self.tab_to(label);
        self.key(Key::Enter, Modifiers::NONE);
    }
    pub fn state(&self) -> Value {
        let mut buffers: Vec<_> = self
            .app
            .manuscript
            .writing_buffers()
            .into_iter()
            .map(|b| (b.path().to_owned(), b.identity().to_owned()))
            .collect();
        buffers.sort();
        let sources: BTreeMap<_, _> = self.app.project.sources().into_iter().collect();
        let docs: BTreeMap<_, _> = self
            .app
            .project
            .authoring_documents
            .iter()
            .map(|(p, d)| (p.clone(), d.bytes().to_vec()))
            .collect();
        json!({"baseline":self.app.project.content_baseline(),"sources":sources,"docs":docs,"buffers":buffers,
            "history":[self.app.history.len(),self.app.redo.len(),self.app.search_state.undo.len(),self.app.search_state.redo.len()],
            "version":self.app.version,"dirty":self.app.project.is_dirty()})
    }
    pub fn fields(&self) -> BTreeMap<String, String> {
        self.app.manuscript.runtime_drafts(&self.app.project.root)
    }
    pub fn literal(&self) -> String {
        let fields = self.fields();
        let request: Value = serde_json::from_str(fields.values().next().unwrap()).unwrap();
        request["operation"]["draft"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    pub fn current_migration(&self) -> worldline_core::manuscript::DialogueEditPlan {
        let plan = self.current_plan();
        assert!(plan.request.enable_language_1_11);
        assert_eq!(self.app.project.language_version(), "1.9");
        assert!(plan.migration.is_some());
        plan
    }
    pub fn current_plan(&self) -> worldline_core::manuscript::DialogueEditPlan {
        let fields = self.fields();
        let request: worldline_core::manuscript::DialogueEditRequest =
            serde_json::from_str(fields.values().next().unwrap()).unwrap();
        let buffer = self.app.manuscript.writing_buffers().pop().unwrap();
        let plan = self
            .app
            .project
            .preview_dialogue_edit(&buffer, &request)
            .unwrap();
        assert!(
            self.app
                .manuscript
                .writing_view
                .dialogue_plan_is_current(buffer.path(), &plan),
            "UI must still hold this exact request and migration digest"
        );
        plan
    }
    pub fn offset(&self) -> f32 {
        self.app.manuscript.scroll_y
    }
    pub fn reading_clip(&self) -> Rect {
        self.reading_domain
            .expect("capture exact READ clip while its label is visible")
            .1
    }
    pub fn reading_focus_visible(&self, out: &egui::FullOutput) -> bool {
        let (owner, clip) = self
            .reading_domain
            .expect("capture exact READ clip while its label is visible");
        self.ctx.memory(|m| m.focused()) == Some(owner)
            && has_viewport_focus_paint(&self.ctx, out, clip)
    }
}
