//! 完整应用外壳的真实键盘事件回归；合成 egui 结果不代表原生桌面验收。
use super::*;
use egui::{Key, Modifiers, Pos2, Vec2};

const EXTRA: &str = "额外资料能力与兼容要求";
const PREVIEW: &str = "预览全稿兼容影响";
const ACK: &str = "我已查看兼容影响；旧存档/检查点须匹配指纹，入口轨迹须重新严格验证";
const APPLY: &str = "确认启用（不自动保存）";
const CANCEL: &str = "取消 / 保留当前设置";
const AUTHOR_INPUT: &str = "尚未提交的译文 JSON😀\n原样保留";

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    physical: Vec2,
    scale: f32,
    time: f64,
}

impl Harness {
    fn new(compact: bool) -> Self {
        let (ctx, mut app) = app();
        let scale = if compact { 2.0 } else { 1.0 };
        app.personal.settings.appearance.ui_scale = scale;
        app.personal.settings.appearance.reduce_motion = true;
        app.localization_ui.exchange_json = AUTHOR_INPUT.into();
        let mut h = Self {
            ctx,
            app,
            physical: if compact {
                vec2(763.0, 541.0)
            } else {
                vec2(1280.0, 800.0)
            },
            scale,
            time: 0.0,
        };
        let shell = h.settle();
        assert!(shell
            .shapes
            .iter()
            .any(|shape| find(&shape.shape, "保存全部").is_some()));
        h.app.open_capabilities();
        h.settle();
        h.assert_bounds();
        h
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let screen = Rect::from_min_size(Pos2::ZERO, self.physical / self.ctx.pixels_per_point());
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(Modifiers::NONE);
        let mut raw = RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            modifiers,
            events,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.outer_rect = Some(screen);
        viewport.focused = Some(true);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        })
    }

    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..5 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    fn assert_bounds(&self) {
        assert_eq!(self.ctx.zoom_factor(), self.scale);
        assert_eq!(self.ctx.pixels_per_point(), self.scale);
        assert_eq!(self.ctx.native_pixels_per_point(), Some(1.0));
        assert_eq!(self.ctx.screen_rect().size(), self.physical / self.scale);
        let window = self
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::new("language-capabilities")))
            .unwrap();
        assert!(
            self.ctx.screen_rect().expand(0.5).contains_rect(window),
            "完整能力窗口必须位于屏幕内：{window:?}"
        );
    }

    fn focused(&self) -> egui::Id {
        self.ctx
            .memory(|memory| memory.focused())
            .expect("真实键盘操作须保留焦点")
    }

    fn complete_focus(&self, label: &str) -> egui::Response {
        let response = self
            .ctx
            .read_response(self.focused())
            .expect("焦点控件实际存在");
        assert!(response.has_focus(), "{label} 应拥有焦点");
        assert!(
            self.ctx.screen_rect().contains_rect(response.rect)
                && response.interact_rect.contains_rect(response.rect),
            "{label} 的完整焦点控件必须可见可操作；rect={:?}, interact={:?}, screen={:?}",
            response.rect,
            response.interact_rect,
            self.ctx.screen_rect()
        );
        response
    }

    fn key(&mut self, key: Key, reverse: bool) -> Vec<egui::output::OutputEvent> {
        let mut events = Vec::new();
        for pressed in [true, false] {
            let output = self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            }]);
            events.extend(output.platform_output.events);
        }
        self.settle();
        events
    }

    #[track_caller]
    fn tab(&mut self, label: &str, reverse: bool) -> egui::Id {
        let before = self.ctx.memory(|memory| memory.focused());
        let events = self.key(Key::Tab, reverse);
        let after = self.ctx.memory(|memory| memory.focused()).and_then(|id| {
            self.ctx
                .read_response(id)
                .map(|response| (id, response.rect, response.sense))
        });
        assert!(events.iter().any(|event| matches!(event,
            egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label))),
            "真实 {}Tab 应到达 {label}，事件：{events:?}，physical={:?}，before={before:?}，after={after:?}",
            if reverse { "Shift+" } else { "" }, self.physical);
        self.complete_focus(label).id
    }

    fn tab_to(&mut self, label: &str) {
        for _ in 0..24 {
            let events = self.key(Key::Tab, false);
            if events.iter().any(|event| matches!(event,
                egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label))) {
                self.complete_focus(label);
                return;
            }
        }
        panic!("真实 Tab 未到达 {label}");
    }

    fn select_113(&mut self) {
        let language = self.complete_focus("初始语言").id;
        self.key(Key::Enter, false);
        assert!(egui::ComboBox::is_open(&self.ctx, language));
        let title = language_capabilities()
            .iter()
            .find(|capability| capability.version == LanguageVersion::V1_13)
            .unwrap()
            .title;
        self.tab_to(title);
        self.key(Key::Enter, false);
        assert_eq!(
            self.app.capability_ui.as_ref().unwrap().target,
            LanguageVersion::V1_13
        );
        assert!(!egui::ComboBox::is_open(&self.ctx, language));
        assert_eq!(self.focused(), language);
        self.complete_focus("选择完成后的语言");
    }

    fn wheel_to_bottom(&mut self) {
        let point = self
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::new("language-capabilities")))
            .unwrap()
            .center();
        self.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -10_000.0),
                modifiers: Modifiers::NONE,
            },
        ]);
        for _ in 0..90 {
            self.frame(vec![]);
        }
        self.settle();
    }

    fn click_visible(&mut self, label: &str) {
        let output = self.settle();
        let point = output
            .shapes
            .iter()
            .find_map(|shape| {
                let point = find(&shape.shape, label)?;
                (shape.clip_rect.contains(point) && self.ctx.screen_rect().contains(point))
                    .then_some(point)
            })
            .unwrap_or_else(|| panic!("鼠标目标必须已经可见：{label}"));
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }

    fn assert_author_input(&self) {
        assert_eq!(self.app.localization_ui.exchange_json, AUTHOR_INPUT);
        assert!(!self.app.project.root.exists(), "能力操作不能隐式保存");
    }
}

#[test]
fn short_200_tab_and_shift_tab_reveal_complete_controls_without_pointer_help() {
    let mut h = Harness::new(true);
    let baseline = h.app.project.content_baseline();
    let language = h.complete_focus("初始语言").id;
    let extra = h.tab(EXTRA, false);
    let preview = h.tab(PREVIEW, false);
    let cancel = h.tab(CANCEL, false);
    assert_ne!(preview, cancel);
    assert_eq!(h.tab(PREVIEW, true), preview);
    assert_eq!(h.tab(EXTRA, true), extra);
    h.key(Key::Tab, true);
    assert_eq!(h.focused(), language);
    h.complete_focus("Shift+Tab 返回语言");
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.assert_author_input();
}

#[test]
fn short_200_reopen_reveals_language_after_previous_pointer_scroll_and_cancel() {
    let mut h = Harness::new(true);
    let baseline = h.app.project.content_baseline();
    let language = h.complete_focus("初始语言").id;
    // 原生复现的前置动作：作者主动滚到底部并取消，不是补救键盘可见性。
    h.wheel_to_bottom();
    h.click_visible(CANCEL);
    assert!(h.app.capability_ui.is_none());
    h.app.open_capabilities();
    h.settle();
    assert_eq!(h.focused(), language);
    h.complete_focus("重新打开后的语言");
    h.assert_bounds();
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.assert_author_input();
}

#[test]
fn short_200_later_pointer_scroll_is_not_pulled_back_to_stale_keyboard_focus() {
    let mut h = Harness::new(true);
    let baseline = h.app.project.content_baseline();
    let language = h.complete_focus("初始语言").id;
    let original = h.ctx.read_response(language).unwrap().rect;
    h.wheel_to_bottom();
    let after_pointer = h.ctx.read_response(language).unwrap().rect;
    assert!(
        after_pointer.top() < original.top() - 40.0,
        "真实滚轮须改变视图"
    );
    assert_eq!(h.focused(), language);
    h.settle();
    let later = h.ctx.read_response(language).unwrap().rect;
    assert!(
        (later.top() - after_pointer.top()).abs() < 0.5,
        "旧焦点不能强拉回文面"
    );
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.assert_author_input();
}

#[test]
fn keyboard_preview_acknowledge_confirm_remain_visible_and_keep_author_input_at_both_sizes() {
    for compact in [true, false] {
        let mut h = Harness::new(compact);
        let sources = h.app.project.sources();
        let baseline = h.app.project.content_baseline();
        h.select_113();
        h.tab(EXTRA, false);
        let preview = h.tab(PREVIEW, false);
        h.key(Key::Enter, false);
        assert_eq!(h.focused(), preview, "预览结果不能改变原按钮身份");
        assert!(
            h.app
                .capability_ui
                .as_ref()
                .unwrap()
                .plan
                .as_ref()
                .unwrap()
                .can_apply
        );
        assert_eq!(h.app.project.content_baseline(), baseline);
        h.tab(ACK, false);
        h.key(Key::Space, false);
        assert!(h.app.capability_ui.as_ref().unwrap().acknowledged);
        h.tab(APPLY, false);
        h.tab(ACK, true);
        h.tab(PREVIEW, true);
        h.tab(ACK, false);
        h.tab(APPLY, false);
        h.assert_bounds();
        h.key(Key::Enter, false);
        assert!(h.app.capability_ui.is_none());
        assert_eq!(h.app.project.language_version(), "1.13");
        assert_eq!(h.app.project.sources(), sources);
        assert_eq!(h.app.history.len(), 1);
        h.assert_author_input();
    }
}

#[test]
fn keyboard_preview_cancel_and_guarded_escape_keep_original_project_and_author_input() {
    for compact in [true, false] {
        let mut h = Harness::new(compact);
        let sources = h.app.project.sources();
        let baseline = h.app.project.content_baseline();
        let language = h.complete_focus("初始语言").id;
        h.key(Key::Enter, false);
        assert!(egui::ComboBox::is_open(&h.ctx, language));
        h.key(Key::Escape, false);
        assert!(!egui::ComboBox::is_open(&h.ctx, language));
        assert!(h.app.capability_ui.is_some());
        h.select_113();
        h.tab(EXTRA, false);
        h.tab(PREVIEW, false);
        h.key(Key::Enter, false);
        h.tab(ACK, false);
        h.key(Key::Space, false);
        h.key(Key::Escape, false);
        assert!(
            h.app.capability_ui.as_ref().unwrap().acknowledged,
            "受保护表单仍须明确取消，Escape 不丢弃兼容确认"
        );
        h.tab(APPLY, false);
        h.tab(CANCEL, false);
        h.key(Key::Enter, false);
        assert!(h.app.capability_ui.is_none());
        assert_eq!(h.app.project.language_version(), "1.9");
        assert_eq!(h.app.project.sources(), sources);
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert!(h.app.history.is_empty());
        h.assert_author_input();
    }
}

#[test]
fn keyboard_feature_only_19_localization_enable_preserves_footer_ids_and_author_input() {
    for compact in [true, false] {
        let mut h = Harness::new(compact);
        let sources = h.app.project.sources();
        let baseline = h.app.project.content_baseline();
        h.complete_focus("初始语言");
        h.tab(EXTRA, false);
        let preview = h.tab(PREVIEW, false);
        let cancel = h.tab(CANCEL, false);
        assert_eq!(h.tab(PREVIEW, true), preview);
        h.tab(EXTRA, true);
        h.key(Key::Enter, false);
        let mut features = Vec::new();
        for feature in feature_capabilities() {
            let label = format!("{} · {}", feature.title, feature.id);
            features.push((label.clone(), h.tab(&label, false)));
        }
        h.key(Key::Space, false);
        assert_eq!(
            h.app.capability_ui.as_ref().unwrap().features,
            BTreeSet::from(["content.localization.v1".to_owned()])
        );
        for (label, id) in features.iter().rev().skip(1) {
            assert_eq!(h.tab(label, true), *id);
        }
        h.tab(EXTRA, true);
        for (label, id) in &features {
            assert_eq!(h.tab(label, false), *id);
        }
        if !compact {
            // 宽窗正文独立滚动：egui 的可聚焦竖滚条位于最后一项和固定 footer 之间。
            let events = h.key(Key::Tab, false);
            assert!(events.is_empty(), "egui 滚条没有按钮的 FocusGained 标签");
            let scrollbar = h.complete_focus("宽窗正文纵滚动条");
            assert!(scrollbar.sense.senses_click() && scrollbar.sense.senses_drag());
            assert!(scrollbar.rect.width() <= h.ctx.style().spacing.scroll.bar_width);
            assert!(scrollbar.rect.height() > 100.0);
            assert_ne!(scrollbar.id, features.last().unwrap().1);
            assert_ne!(scrollbar.id, preview);
            assert_ne!(scrollbar.id, cancel);
        }
        assert_eq!(h.tab(PREVIEW, false), preview);
        h.key(Key::Enter, false);
        assert_eq!(h.focused(), preview, "计划内容不得移动预览按钮身份");
        let plan = h.app.capability_ui.as_ref().unwrap().plan.as_ref().unwrap();
        assert!(plan.can_apply);
        assert_eq!(plan.current_language, LanguageVersion::V1_9);
        assert_eq!(plan.target_language, LanguageVersion::V1_9);
        assert_eq!(h.app.project.content_baseline(), baseline);
        let ack = h.tab(ACK, false);
        h.key(Key::Space, false);
        assert_eq!(h.focused(), ack);
        let apply = h.tab(APPLY, false);
        assert_eq!(h.tab(CANCEL, false), cancel, "确认控件出现不能移动取消身份");
        assert_eq!(h.tab(APPLY, true), apply);
        h.key(Key::Enter, false);
        assert!(h.app.capability_ui.is_none());
        assert_eq!(h.app.project.language_version(), "1.9");
        assert!(h.app.project.compile_options().localization_ids);
        assert_eq!(
            h.app.project.required_features(),
            vec!["content.localization.v1"]
        );
        assert_eq!(h.app.project.sources(), sources);
        assert_eq!(h.app.history.len(), 1);
        h.assert_author_input();
    }
}
