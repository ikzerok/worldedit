//! 组合和平台生命周期分开：真实文本 owner、结束帧及直接 Commit 的动作边界。
use super::*;

fn key(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn schema_full_app_ime_preedit_finishing_frames_keep_draft_and_owner() {
    for commit in [false, true] {
        let mut h = FullApp::new();
        h.open_from_real_character_entry();
        let target = h.bind_and_fill();
        let field = h.focused().id;
        let baseline = h.app.project.content_baseline();
        h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
        h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
        let composing = h.app.schema_authoring.sessions[&target].values.clone();
        // 候选 Escape/Tab 由真实事件触发；即便 egui 短暂清焦点，也须保住 owner。
        for navigation in [Key::Escape, Key::Tab] {
            h.press(navigation, Modifiers::NONE);
            h.settle();
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field));
            assert!(h.app.schema_authoring.open);
            assert_eq!(h.app.schema_authoring.sessions[&target].values, composing);
        }
        let ending = if commit {
            egui::ImeEvent::Commit("完成".into())
        } else {
            egui::ImeEvent::Disabled
        };
        h.frame(vec![
            Event::Ime(egui::ImeEvent::Preedit("候选".into())),
            Event::Ime(ending),
            key(Key::Escape, true),
        ]);
        assert!(h.app.schema_authoring.open, "结束帧不得关闭工作表");
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field));
        h.frame(vec![key(Key::Escape, false)]);
        h.settle();
        if commit {
            let Some(PropertyValue::Str(value)) =
                &h.app.schema_authoring.sessions[&target].values["memo_id"]
            else {
                panic!("组合提交必须留在原文本字段");
            };
            assert!(value.contains("完成"));
        }
        let values = h.app.schema_authoring.sessions[&target].values.clone();
        h.press(Key::Escape, Modifiers::NONE);
        h.settle();
        assert!(!h.app.schema_authoring.open, "下一正常帧应解除结束保护");
        assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert!(h.app.history.is_empty());
    }
}

fn direct_commit_enter(h: &mut FullApp) {
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Commit(String::new())),
        key(Key::Enter, true),
    ]);
    h.frame(vec![key(Key::Enter, false)]);
    h.settle();
}

#[test]
fn schema_full_app_ime_direct_commit_blocks_apply_source_and_close() {
    let mut h = FullApp::new();
    h.open_from_real_character_entry();
    let target = h.bind_and_fill();
    let baseline = h.app.project.content_baseline();
    h.tab_to("预览字段修改", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    h.tab_to("确认应用这次运行指纹变更", WidgetType::Checkbox, false);
    h.press(Key::Space, Modifiers::NONE);
    h.tab_to("应用到工程", WidgetType::Button, true);
    assert!(h.focused().enabled());
    direct_commit_enter(&mut h);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty(), "直接Commit同帧不能应用");
    assert!(h.app.schema_authoring.open);

    h.tab_to("取消（保留输入）", WidgetType::Button, false);
    direct_commit_enter(&mut h);
    assert!(h.app.schema_authoring.open, "直接Commit同帧不能取消");
    h.tab_to("Close window", WidgetType::Button, false);
    direct_commit_enter(&mut h);
    assert!(h.app.schema_authoring.open, "直接Commit同帧不能关闭窗框");

    h.tab_to("字段身份与来源", WidgetType::CollapsingHeader, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to("查看现值来源", WidgetType::Link, false);
    direct_commit_enter(&mut h);
    assert!(h.app.schema_authoring.open);
    assert!(
        !h.app.schema_authoring.source_return,
        "直接Commit同帧不能转来源"
    );
    assert_eq!(h.app.schema_authoring.active.as_ref(), Some(&target));
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
}

#[test]
fn schema_full_app_ime_first_preedit_on_tab_focus_keeps_owner() {
    let mut h = FullApp::new();
    h.open_from_real_character_entry();
    let target = h.bind_and_fill();
    let field = fields::input_id(&target, "memo_id");
    let baseline = h.app.project.content_baseline();
    h.tab_to("对象诊断与关联来源", WidgetType::CollapsingHeader, true);
    assert_ne!(h.focused().id, field);
    h.frame(vec![
        key(Key::Tab, true),
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("首批候选".into())),
    ]);
    assert_eq!(
        h.ctx.memory(|memory| memory.focused()),
        Some(field),
        "首批事件须真的从诊断header移到文本字段: {}",
        h.trace.join("\n")
    );
    let Some(PropertyValue::Str(value)) =
        &h.app.schema_authoring.sessions[&target].values["memo_id"]
    else {
        panic!("真实字段应接收预编辑");
    };
    assert!(
        value.contains("首批候选"),
        "egui须已实际接收本批Preedit: {value:?}"
    );
    h.frame(vec![key(Key::Tab, false)]);
    h.settle();
    let composing = h.app.schema_authoring.sessions[&target].values.clone();
    h.press(Key::Tab, Modifiers::NONE);
    h.settle();
    assert_eq!(
        h.ctx.memory(|memory| memory.focused()),
        Some(field),
        "首次Preedit与Tab同批时也须追踪真实owner，后续Tab不能离开: {}",
        h.trace.join("\n")
    );
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(h.app.schema_authoring.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field));
    assert_eq!(h.app.schema_authoring.sessions[&target].values, composing);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
}
