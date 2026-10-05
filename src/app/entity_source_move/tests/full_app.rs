//! 完整 App 调度复现：后台实体表单、侧栏、主源码区和移源窗口同时参与真实 egui 帧。
use super::*;
use egui::{Event, Key, Modifiers, Rect, Vec2};
struct FullApp {
    ctx: egui::Context,
    app: WorldeditApp,
    size: Vec2,
}
impl FullApp {
    fn new(size: Vec2) -> Self {
        let (ctx, mut app) = fixture();
        for name in [
            "人物/档案员与证人.wl",
            "故事/听证与倒流.wl",
            "档案/航路索引.wl",
        ] {
            let path = app.project.add_file(std::path::Path::new(name)).unwrap();
            app.project
                .set_text(&path, "// 已存在的活动源码\n".into())
                .unwrap();
        }
        app.project.save().unwrap();
        app.recompile();
        app.tab = Tab::Edit;
        app.personal.pending_restore = false;
        assert_eq!(app.entity_move_targets().len(), 6);
        assert_eq!(app.project.documents.len(), 8);
        Self { ctx, app, size }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest()),
        )
    }
    fn settle(&mut self) {
        for _ in 0..12 {
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
    }
    fn command(&mut self, commands_only: bool, query: &str) {
        self.press(
            Key::P,
            if commands_only {
                Modifiers::COMMAND | Modifiers::SHIFT
            } else {
                Modifiers::COMMAND
            },
        );
        self.frame(vec![Event::Text(query.into())]);
        self.settle();
        assert!(self.app.command_palette.open);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
    }
    fn trace(&self) -> String {
        let form = self.app.entity_source_move_form.as_ref();
        format!(
            "focused={:?}, layers={:?}, target_ids={:?}, selected={:?}, ime={}/{}/{}",
            self.ctx.memory(|memory| memory.focused()),
            self.app.command_palette.focus_stack,
            form.map(|form| &form.target_focus_ids),
            form.map(|form| &form.destination),
            self.app.ime_composing,
            self.app.command_palette.ime,
            self.app.command_palette.ime_frame
        )
    }
}
impl Drop for FullApp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.app.project.root);
    }
}
#[test]
fn entity_move_full_app_command_from_open_entity_reaches_sixth_target_and_preview() {
    for size in [
        egui::vec2(1188.0, 848.0),
        egui::vec2(1280.0, 800.0),
        egui::vec2(1040.0, 660.0),
        egui::vec2(800.0, 600.0),
    ] {
        let mut harness = FullApp::new(size);
        harness.settle();
        harness.command(false, "lighthouse");
        assert_eq!(
            harness
                .app
                .entity_editor
                .as_ref()
                .unwrap()
                .original
                .as_deref(),
            Some("lighthouse")
        );
        harness.command(true, "移到其他源码");
        assert!(!harness.app.command_palette.open);
        let form = harness.app.entity_source_move_form.as_ref().unwrap();
        assert_eq!(form.target_focus_ids.len(), 6, "{}", harness.trace());
        assert!(
            harness
                .ctx
                .memory(|memory| memory.focused())
                .is_some_and(|id| form.target_focus_ids.contains(&id)),
            "移源新窗应持有键盘焦点：{}",
            harness.trace()
        );
        harness.press(Key::Enter, Modifiers::NONE);
        let form = harness.app.entity_source_move_form.as_ref().unwrap();
        assert!(
            form.plan
                .as_ref()
                .is_some_and(|plan| plan.changes.is_empty()),
            "当前来源 Enter 应产生明确零变化计划：{}",
            harness.trace()
        );
        // 取消并重开后仍有后台干净表单；源是第5项，目标是可滚动的第6项。
        harness.press(Key::Escape, Modifiers::NONE);
        assert!(harness.app.entity_source_move_form.is_none());
        harness.command(true, "移到其他源码");
        harness.press(Key::ArrowDown, Modifiers::NONE);
        harness.settle();
        assert_eq!(
            harness
                .app
                .entity_source_move_form
                .as_ref()
                .unwrap()
                .destination,
            harness.app.project.root.join(TARGET),
            "第6目标必须可达：{}",
            harness.trace()
        );
        harness.press(Key::Enter, Modifiers::NONE);
        let form = harness.app.entity_source_move_form.as_ref().unwrap();
        assert!(
            form.plan
                .as_ref()
                .is_some_and(|plan| plan.changes.len() == 2),
            "完整App调度下必须能预览两侧：{}",
            harness.trace()
        );
        assert!(harness.app.history.is_empty());
        harness.press(Key::Escape, Modifiers::NONE);
        assert!(harness.app.entity_source_move_form.is_none());
        assert!(harness.app.entity_editor.is_some());
        assert!(!harness.app.project.is_dirty());
    }
}

#[test]
fn entity_move_full_app_animated_backgrounds_keep_source_list_focus() {
    for tab in [Tab::Edit, Tab::Timeline, Tab::Catalog] {
        let mut harness = FullApp::new(egui::vec2(1188.0, 848.0));
        harness.app.tab = tab;
        // 基础夹具为确定性测试关闭动画；原生默认窗口不关闭动画。
        harness.ctx.style_mut(|style| style.animation_time = 0.25);
        harness.settle();
        harness.command(false, "lighthouse");
        harness.command(true, "移到其他源码");
        for _ in 0..30 {
            harness.frame(vec![]);
            let form = harness.app.entity_source_move_form.as_ref().unwrap();
            assert!(
                harness
                    .ctx
                    .memory(|memory| memory.focused())
                    .is_some_and(|id| form.target_focus_ids.contains(&id)),
                "背景 {tab:?} 空闲重绘不得丢焦点：{}",
                harness.trace()
            );
        }
        harness.press(Key::ArrowDown, Modifiers::NONE);
        harness.settle();
        assert_eq!(
            harness
                .app
                .entity_source_move_form
                .as_ref()
                .unwrap()
                .destination,
            harness.app.project.root.join(TARGET),
            "动画模式下第6目标必须可达：{}",
            harness.trace()
        );
        harness.press(Key::Enter, Modifiers::NONE);
        assert!(
            harness
                .app
                .entity_source_move_form
                .as_ref()
                .unwrap()
                .plan
                .is_some(),
            "背景 {tab:?} 未生成预览：{}",
            harness.trace()
        );
        assert!(harness.app.history.is_empty());
        harness.press(Key::Escape, Modifiers::NONE);
        assert!(harness.app.entity_source_move_form.is_none());
        assert!(harness.app.entity_editor.is_some());
    }
}

#[test]
fn entity_move_full_app_backend_ime_release_keeps_focus_and_sixth_target_reachable() {
    for ime in [
        egui::ImeEvent::Disabled,
        egui::ImeEvent::Commit(String::new()),
    ] {
        let mut harness = FullApp::new(egui::vec2(1188.0, 848.0));
        harness.ctx.style_mut(|style| style.animation_time = 0.25);
        harness.settle();
        harness.command(false, "lighthouse");
        harness.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
        harness.frame(vec![Event::Text("移到其他源码".into())]);
        harness.settle();
        // 原生已证实的序列：开窗按下Enter，一次空闲帧，后端IME事件伴随Enter释放。
        harness.frame(vec![Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        harness.frame(vec![]);
        let focus = harness.ctx.memory(|memory| memory.focused());
        assert!(focus.is_some_and(|id| harness
            .app
            .entity_source_move_form
            .as_ref()
            .unwrap()
            .target_focus_ids
            .contains(&id)));
        harness.frame(vec![
            Event::Ime(ime),
            Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        assert_eq!(
            harness.ctx.memory(|memory| memory.focused()),
            focus,
            "后端IME收尾不得丢焦点：{}",
            harness.trace()
        );
        assert!(harness
            .app
            .entity_source_move_form
            .as_ref()
            .unwrap()
            .plan
            .is_none());
        assert!(harness.app.history.is_empty());
        harness.press(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(
            harness
                .app
                .entity_source_move_form
                .as_ref()
                .unwrap()
                .destination,
            harness.app.project.root.join(TARGET),
            "IME后第6目标必须可达：{}",
            harness.trace()
        );
        harness.press(Key::Enter, Modifiers::NONE);
        assert!(harness
            .app
            .entity_source_move_form
            .as_ref()
            .unwrap()
            .plan
            .as_ref()
            .is_some_and(|plan| plan.changes.len() == 2));
        assert!(harness.app.history.is_empty());
        harness.press(Key::Escape, Modifiers::NONE);
        assert!(harness.app.entity_editor.is_some());
        assert!(!harness.app.project.is_dirty());
    }
}

#[test]
fn entity_move_full_app_ime_never_applies_or_replays_enter_and_keeps_apply_focus() {
    let mut harness = FullApp::new(egui::vec2(1188.0, 848.0));
    harness.settle();
    harness.command(false, "lighthouse");
    harness.command(true, "移到其他源码");
    harness.press(Key::ArrowDown, Modifiers::NONE);
    harness.press(Key::Enter, Modifiers::NONE);
    let focus = harness
        .app
        .entity_source_move_form
        .as_ref()
        .unwrap()
        .apply_focus;
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), focus);
    let baseline = harness.app.project.content_baseline();
    for ime in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("候选".into()),
        egui::ImeEvent::Commit("候选".into()),
        egui::ImeEvent::Disabled,
    ] {
        harness.frame(vec![
            Event::Ime(ime),
            Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        assert_eq!(harness.app.project.content_baseline(), baseline);
        assert!(harness.app.history.is_empty());
        assert!(harness.app.entity_source_move_form.is_some());
        assert_eq!(
            harness.ctx.memory(|memory| memory.focused()),
            focus,
            "IME期间应用焦点应保留：{}",
            harness.trace()
        );
        harness.frame(vec![Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }
    harness.settle();
    assert_eq!(
        harness.app.project.content_baseline(),
        baseline,
        "组合结束不能回放被忽略的Enter"
    );
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), focus);
    harness.press(Key::Enter, Modifiers::NONE);
    assert!(harness.app.entity_source_move_form.is_none());
    assert_eq!(harness.app.history.len(), 1, "只有新的明确Enter才应用");
    let target_path = harness.app.project.root.join(TARGET);
    assert_eq!(harness.app.active_file, target_path);
    assert_eq!(harness.app.tab, Tab::Edit);
    assert_eq!(
        harness.app.entity_source_navigation,
        Some(("lighthouse".into(), target_path.clone()))
    );
    let object = harness
        .app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("entity", "lighthouse"))
        .unwrap();
    assert_eq!(PathBuf::from(&object.file), target_path);
    let (editor_id, text) = harness.app.source_position_document(&target_path).unwrap();
    let expected_cursor = text
        .split_inclusive('\n')
        .take(object.line.saturating_sub(1) as usize)
        .map(|line| line.chars().count())
        .sum::<usize>();
    let selection = egui::TextEdit::load_state(&harness.ctx, editor_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        selection.primary.index, expected_cursor,
        "真实源码编辑器落在core声明起点"
    );
    assert_eq!(selection.secondary.index, expected_cursor);
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(editor_id)
    );
    assert!(
        harness.app.jump.is_none(),
        "完整App释放帧已消费一次性导航请求"
    );
}

fn visible_text_rect(output: &egui::FullOutput, text: &str) -> Option<Rect> {
    output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(shape) if shape.galley.text() == text => {
                let rect = shape.galley.rect.translate(shape.pos.to_vec2());
                clipped.clip_rect.contains_rect(rect).then_some(rect)
            }
            _ => None,
        })
}

#[test]
fn entity_move_full_app_reveals_new_preview_once_and_respects_manual_scroll() {
    let mut harness = FullApp::new(egui::vec2(1188.0, 848.0));
    harness.settle();
    harness.command(false, "lighthouse");
    harness.command(true, "移到其他源码");
    harness.press(Key::ArrowDown, Modifiers::NONE);
    let before = harness.frame(vec![]);
    let cancel = visible_text_rect(&before, "取消移源").unwrap();
    harness.press(Key::Enter, Modifiers::NONE);
    harness.settle();
    let reviewed = harness.frame(vec![]);
    assert!(
        visible_text_rect(&reviewed, "实体移源 · 精确原文预览").is_some(),
        "新计划必须自动揭示审阅起点"
    );
    let source_raw =
        visible_text_rect(&reviewed, DECLARATION).expect("成功预览后源删除精确原文应在可见区");
    assert_eq!(visible_text_rect(&reviewed, "取消移源"), Some(cancel));
    let form = harness.app.entity_source_move_form.as_ref().unwrap();
    assert!(!form.reveal_preview, "一次性揭示标记已消费");
    let plan = form.plan.as_ref().unwrap();
    let digest = plan.plan_digest.clone();
    let insertion = plan
        .changes
        .iter()
        .find(|change| Some(&change.path) == plan.destination_path.as_ref())
        .unwrap()
        .occurrences[0]
        .after_token
        .clone();
    harness.frame(vec![
        Event::PointerMoved(source_raw.center()),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -100_000.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    harness.settle();
    let scrolled = harness.frame(vec![]);
    assert!(
        visible_text_rect(&scrolled, "实体移源 · 精确原文预览").is_none(),
        "手动向下审阅不能被拉回预览开头"
    );
    let insertion_rect =
        visible_text_rect(&scrolled, &insertion).expect("可手动滚动到目标插入原文");
    assert_eq!(visible_text_rect(&scrolled, "取消移源"), Some(cancel));
    harness.settle();
    let idle = harness.frame(vec![]);
    assert_eq!(
        visible_text_rect(&idle, &insertion),
        Some(insertion_rect),
        "后续空闲帧不重复自动滚动"
    );
    assert!(visible_text_rect(&idle, "实体移源 · 精确原文预览").is_none());
    assert_eq!(
        harness
            .app
            .entity_source_move_form
            .as_ref()
            .unwrap()
            .plan
            .as_ref()
            .unwrap()
            .plan_digest,
        digest
    );
    assert!(harness.app.history.is_empty());
    assert!(!harness.app.project.is_dirty());
}
