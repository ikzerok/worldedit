//! 首次待确认提示与局部树结构变化，必须保留真实键盘控件身份。
use super::*;
use egui::{Event, Id, Key, Modifiers};

struct FullApp {
    ctx: egui::Context,
    app: WorldeditApp,
    source: String,
    baseline: String,
    draft: worldline_core::authoring::EventDraft,
    output: egui::FullOutput,
    height: f32,
}
impl FullApp {
    fn new(after: &str) -> Self {
        let (ctx, mut app) = setup();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let source = format!(
            "let allowed = true\nlet reputation = 3\nlet title = \"访客\"\nevent start after {after}\n  原始正文。\n  -> END\n"
        );
        app.project
            .set_text(&app.active_file.clone(), source.clone())
            .unwrap();
        app.project.save().unwrap();
        app.recompile();
        assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
        app.select_event("start");
        app.tab = crate::app::Tab::Timeline;
        app.personal.pending_restore = false;
        let baseline = app.project.content_baseline();
        let draft = app.event_editor.as_ref().unwrap().draft.clone();
        let mut h = Self {
            ctx,
            app,
            source,
            baseline,
            draft,
            output: Default::default(),
            height: 848.0,
        };
        h.settle();
        assert!(
            !h.pending(),
            "the first edit must insert the pending header"
        );
        h
    }
    fn frame(&mut self, events: Vec<Event>) {
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, self.height),
            )),
            events,
            modifiers,
            focused: true,
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        self.output = self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        });
        self.untouched();
    }
    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }
    fn press(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.settle();
    }
    fn text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.into())]);
        self.settle();
    }
    fn pending(&self) -> bool {
        self.app
            .event_editor
            .as_ref()
            .unwrap()
            .condition_forms
            .has_pending()
    }
    fn untouched(&self) {
        assert_eq!(self.app.project.content_baseline(), self.baseline);
        assert_eq!(
            self.app.project.document(&self.app.active_file).unwrap(),
            self.source
        );
        assert_eq!(
            std::fs::read_to_string(&self.app.active_file).unwrap(),
            self.source
        );
        assert!(
            self.app.event_editor.as_ref().unwrap().draft == self.draft,
            "typing or navigating a condition buffer must not change the event draft"
        );
    }
    fn widgets(&self, label: &str) -> Vec<Id> {
        fn text_rects(shape: &egui::Shape, label: &str, found: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    found.push(text.galley.rect.translate(text.pos.to_vec2()));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        text_rects(shape, label, found);
                    }
                }
                _ => {}
            }
        }
        let mut labels = Vec::new();
        for shape in &self.output.shapes {
            text_rects(&shape.shape, label, &mut labels);
        }
        self.ctx.viewport(|viewport| {
            let mut matches = Vec::new();
            for label in labels {
                // 精确绘制文字所属的最小交互控件，不把可聚焦父滚动区当成输入框。
                if let Some(widget) = viewport
                    .this_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, layer)| layer.iter())
                    .filter(|widget| {
                        widget.enabled
                            && widget.sense.is_focusable()
                            && widget.rect.expand(1.0).contains_rect(label)
                    })
                    .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
                {
                    matches.push((widget.rect.top(), widget.rect.left(), widget.id));
                }
            }
            matches.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            matches.dedup_by_key(|(_, _, id)| *id);
            matches.into_iter().map(|(_, _, id)| id).collect()
        })
    }
    fn one(&self, label: &str) -> Id {
        let ids = self.widgets(label);
        assert_eq!(
            ids.len(),
            1,
            "fixture must identify one actual {label} widget: {ids:?}"
        );
        ids[0]
    }
    fn focus(&self) -> Option<Id> {
        self.ctx.memory(|memory| memory.focused())
    }
    fn seek(&mut self, id: Id, reverse: bool) {
        for _ in 0..180 {
            if self.focus() == Some(id) {
                for _ in 0..24 {
                    self.frame(vec![]);
                }
                let response = self.ctx.read_response(id).unwrap();
                assert!(response.enabled());
                assert!(
                    response
                        .interact_rect
                        .expand(1.0)
                        .contains_rect(response.rect),
                    "whole focused response must be visible: {response:?}"
                );
                assert!(self.ctx.screen_rect().contains_rect(response.rect));
                return;
            }
            self.press(
                Key::Tab,
                if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            );
        }
        panic!(
            "natural Tab did not reach {id:?}; actual focus {:?}",
            self.focus()
        );
    }
    fn number(&self) -> f64 {
        fn find(node: &ConditionNode) -> Option<f64> {
            match node {
                ConditionNode::Compare {
                    value: ConditionLiteral::Num(value),
                    ..
                } => Some(*value),
                ConditionNode::Not { child } => find(child),
                ConditionNode::And { left, right } | ConditionNode::Or { left, right } => {
                    find(left).or_else(|| find(right))
                }
                _ => None,
            }
        }
        find(self.tree()).unwrap()
    }
    fn tree(&self) -> &ConditionNode {
        self.app
            .event_editor
            .as_ref()
            .unwrap()
            .condition_forms
            .entries
            .iter()
            .find(|(_, slot, _)| *slot == ConditionSlot::EventAfter)
            .unwrap()
            .2
            .tree
            .as_ref()
            .unwrap()
    }
    fn number_id(&self) -> Id {
        let (frame, fields) = self
            .ctx
            .data(|data| {
                data.get_temp::<(u64, Vec<(f64, Id)>)>(Id::new("condition-number-test-fields"))
            })
            .expect("actual numeric widgets were rendered");
        assert_eq!(frame + 1, self.ctx.cumulative_frame_nr());
        assert_eq!(
            fields.len(),
            1,
            "one numeric condition literal in this fixture"
        );
        assert_eq!(fields[0].0, self.number());
        fields[0].1
    }
    fn string_id(&self) -> Id {
        self.ctx
            .data(|data| data.get_temp::<Id>(Id::new("condition-literal-test-focus")))
            .unwrap()
    }
    fn menu(&mut self, index: usize, command: &str) {
        let menu = self.widgets("调整子句")[index];
        self.seek(menu, true);
        self.press(Key::Enter, Modifiers::NONE);
        assert!(egui::Popup::is_any_open(&self.ctx));
        let action = self.one(command);
        self.seek(action, false);
        self.press(Key::Enter, Modifiers::NONE);
        assert!(!egui::Popup::is_any_open(&self.ctx));
    }
}

#[test]
fn condition_ui_first_pending_numeric_input_keeps_id_focus_multidigit_and_next_tab() {
    let mut h = FullApp::new("reputation >= 3");
    let number = h.number_id();
    h.seek(number, false);
    let range = egui::TextEdit::load_state(&h.ctx, number)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        range.primary.index.abs_diff(range.secondary.index),
        1,
        "Tab selects the original 3"
    );
    assert!(!h.pending());
    h.text("2");
    assert!(h.pending());
    assert_eq!(h.number(), 2.0);
    let first_id = h.number_id();
    let first_focus = h.focus();
    // 不重新聚焦；第二位确实在后续帧送到同一原生编辑会话。
    h.text("5");
    assert_eq!(h.number(), 25.0,
        "split input must retain both digits; before={number:?}, after first={first_id:?}, first focus={first_focus:?}, now={:?}", h.focus());
    assert_eq!(first_id, number);
    assert_eq!(first_focus, Some(number));
    assert_eq!(h.number_id(), number);
    assert_eq!(h.focus(), Some(number));
    h.press(Key::Tab, Modifiers::NONE);
    let direction = h.one("字面量在左侧（保持比较方向）");
    assert_eq!(
        h.focus(),
        Some(direction),
        "Tab must continue inside the condition, not restart in chrome"
    );
    h.press(Key::Tab, Modifiers::NONE);
    assert_eq!(h.focus(), Some(h.one("删除整个条件")));
    h.press(Key::Tab, Modifiers::NONE);
    let preview = crate::chrome::action_layout::trace(&h.ctx)
        .into_iter()
        .find(|response| response.role == "condition/前置条件 after/preview")
        .unwrap();
    assert_eq!(h.focus(), Some(preview.id));
    h.untouched();
}

#[test]
fn condition_ui_first_pending_string_and_comparison_controls_keep_identity() {
    let mut h = FullApp::new("title == \"\"");
    let string = h.string_id();
    let variable = crate::chrome::action_layout::trace(&h.ctx)
        .into_iter()
        .find(|response| response.role == "condition/variable-picker")
        .unwrap()
        .id;
    h.seek(string, false);
    let operation = h.one("等于 ==");
    h.text("甲");
    assert!(h.pending());
    assert_eq!(h.string_id(), string);
    assert_eq!(h.focus(), Some(string));
    h.text("乙");
    assert_eq!(h.focus(), Some(string));
    assert!(
        matches!(h.tree(), ConditionNode::Compare { value: ConditionLiteral::Str(text), .. } if text == "甲乙")
    );
    let later = crate::chrome::action_layout::trace(&h.ctx)
        .into_iter()
        .find(|response| response.role == "condition/variable-picker")
        .unwrap()
        .id;
    assert_eq!(later, variable);
    assert_eq!(h.one("等于 =="), operation);
    h.press(Key::Tab, Modifiers::NONE);
    assert_eq!(h.focus(), Some(h.one("字面量在左侧（保持比较方向）")));
    h.untouched();
}

#[test]
fn condition_ui_wrapping_and_unwrapping_left_clause_preserves_right_input_identity() {
    let mut h = FullApp::new("allowed and reputation >= 3");
    let number = h.number_id();
    h.seek(number, false);
    h.menu(1, "对此子句取反");
    assert!(h.pending());
    assert!(
        matches!(h.tree(), ConditionNode::And { left, .. } if matches!(left.as_ref(), ConditionNode::Not { .. }))
    );
    assert_eq!(
        h.number_id(),
        number,
        "adding a left child must not recreate the unchanged right input"
    );
    h.seek(number, false);
    h.menu(1, "移除取反，保留原项");
    assert!(!h.pending());
    assert_eq!(
        h.number_id(),
        number,
        "removing the left wrapper and pending header must retain the right input"
    );
    h.seek(number, false);
    h.text("2");
    h.text("5");
    assert_eq!(h.number(), 25.0);
    assert_eq!(h.number_id(), number);
    assert_eq!(h.focus(), Some(number));
    h.untouched();
}

#[test]
fn condition_ui_first_pending_numeric_stays_visible_and_later_wheel_is_not_pulled_back() {
    fn geometry(h: &FullApp, number: Id) -> (egui::Response, egui::Rect) {
        let response = h
            .ctx
            .read_response(number)
            .expect("same numeric response still exists");
        let clip = crate::chrome::action_layout::trace(&h.ctx)
            .into_iter()
            .find(|action| action.role == "condition/前置条件 after/preview")
            .expect("same after condition scroll host")
            .clip;
        (response, clip)
    }
    let mut h = FullApp::new("reputation >= 3");
    let number = h.number_id();
    h.seek(number, false);
    // 仅调整真实客户端大小，使原输入完整可见且靠近视口下沿；不修改滚动状态。
    for _ in 0..8 {
        let (response, clip) = geometry(&h, number);
        let remaining = clip.bottom() - response.rect.bottom();
        if clip.contains_rect(response.rect) && (2.0..=12.0).contains(&remaining) {
            break;
        }
        h.height += response.rect.bottom() + 6.0 - clip.bottom();
        assert!(
            (300.0..=1100.0).contains(&h.height),
            "bounded real client height: {}",
            h.height
        );
        for _ in 0..24 {
            h.frame(vec![]);
        }
    }
    let (before, before_clip) = geometry(&h, number);
    assert_eq!(h.focus(), Some(number));
    assert!(!h.pending());
    assert!(
        before_clip.contains_rect(before.rect),
        "whole numeric before first edit: {before:?}, clip={before_clip:?}"
    );
    assert!(
        (2.0..=12.0).contains(&(before_clip.bottom() - before.rect.bottom())),
        "real input must start near viewport bottom: {before:?}, clip={before_clip:?}"
    );
    assert!(h.ctx.screen_rect().contains_rect(before.rect));
    h.text("2");
    assert!(h.pending());
    assert_eq!(h.number(), 2.0);
    assert_eq!(h.number_id(), number);
    assert_eq!(h.focus(), Some(number));
    let (edited, edited_clip) = geometry(&h, number);
    assert!(edited_clip.contains_rect(edited.rect),
        "first pending header must leave the entire live input visible: before={before:?}, before clip={before_clip:?}; after={edited:?}, after clip={edited_clip:?}");
    assert!(edited.interact_rect.contains_rect(edited.rect));
    assert!(h.ctx.screen_rect().contains_rect(edited.rect));
    // 一个更新的真实滚轮意图可以离开当前输入，旧焦点不能反复把它拉回来。
    h.frame(vec![
        Event::PointerMoved(edited.rect.center()),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -10_000.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    for _ in 0..90 {
        h.frame(vec![]);
    }
    let (scrolled, scrolled_clip) = geometry(&h, number);
    assert_eq!(h.focus(), Some(number));
    assert_eq!(h.number_id(), number);
    assert_eq!(h.number(), 2.0);
    assert!(
        scrolled.rect.top() < edited.rect.top() - 40.0,
        "wheel must actually leave the edited field: {edited:?} -> {scrolled:?}"
    );
    assert!(
        !scrolled_clip.contains_rect(scrolled.rect),
        "the user's scroll may deliberately move the live field outside the viewport"
    );
    for _ in 0..12 {
        h.frame(vec![]);
    }
    let (settled, _) = geometry(&h, number);
    assert!(
        (settled.rect.top() - scrolled.rect.top()).abs() < 0.5,
        "old focus must not pull back later wheel: {scrolled:?} -> {settled:?}"
    );
    assert_eq!(h.focus(), Some(number));
    assert_eq!(h.number(), 2.0);
    h.untouched();
}
