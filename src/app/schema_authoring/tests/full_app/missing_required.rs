//! 缺失必填值使用真实“添加”动作进入；既存零/false/空串不是缺失。
use super::*;

fn focus_empty_field(h: &mut FullApp, id: egui::Id) {
    for _ in 0..120 {
        if h.ctx.memory(|memory| memory.focused()) == Some(id) {
            let response = h.ctx.read_response(id).expect("真实字段响应");
            let (_, viewport, _) = h.app.schema_authoring.body_scroll.unwrap();
            assert!(response.enabled());
            assert!(egui::TextEdit::load_state(&h.ctx, id).is_some());
            assert!(
                viewport.contains_rect(response.rect),
                "空字段须完整滚入正文: {:?}, {viewport:?}",
                response.rect
            );
            assert!(response.interact_rect.contains_rect(response.rect));
            assert_eq!(
                h.ctx.layer_id_at(response.rect.center()),
                Some(response.layer_id)
            );
            h.assert_sheet_focus();
            return;
        }
        h.press(Key::Tab, Modifiers::NONE);
        h.settle();
    }
    panic!("空字段键盘不可达：{}", h.trace.join("\n"));
}

#[test]
fn schema_full_app_missing_required_add_fill_preview_and_apply() {
    for zoom_twice in [false, true] {
        let mut h = FullApp::new();
        let schema = h
            .app
            .project
            .add_file(std::path::Path::new("schema.wl"))
            .unwrap();
        h.app.project.set_text(&schema, "schema character_data for character\n  field label_id label text required\n  field count_id count number required\n  field public_id public boolean required\n  field note_id note text required\n".into()).unwrap();
        h.app.project.set_text(&h.app.active_file.clone(), "character lead as \"人物验收\"\n  property count = 0\n  property public = false\n  property note = \"\"\n  property custom = \"人物未知属性保留\"\nevent start\n  正文。\n  -> END\n".into()).unwrap();
        h.app.recompile();
        let baseline = h.app.project.content_baseline();
        let entry = h.open_from_real_character_entry();
        if zoom_twice {
            for _ in 0..10 {
                h.press(Key::Equals, Modifiers::COMMAND);
            }
            h.settle();
            assert_eq!(h.ctx.pixels_per_point(), 2.0);
        }
        h.assert_frame_visible();
        let combo = h.tab_to("未绑定", WidgetType::ComboBox, false);
        h.press(Key::Enter, Modifiers::NONE);
        h.tab_to("character_data", WidgetType::Button, false);
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert!(!egui::ComboBox::is_open(&h.ctx, combo));
        h.tab_to("预览所选绑定", WidgetType::Button, false);
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        let target = TargetRef::new("character", "lead");
        let form = &h.app.schema_authoring.sessions[&target];
        assert_eq!(form.values["label_id"], None);
        assert_eq!(form.values["count_id"], Some(PropertyValue::Num(0.0)));
        assert_eq!(form.values["public_id"], Some(PropertyValue::Bool(false)));
        assert_eq!(
            form.values["note_id"],
            Some(PropertyValue::Str(String::new()))
        );
        assert!(form
            .projection
            .fields
            .iter()
            .find(|field| field.id == "label_id")
            .unwrap()
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SCH004"));
        assert_eq!(h.app.project.content_baseline(), baseline);

        let add = h.tab_to("添加此字段", WidgetType::Button, false);
        assert_eq!(h.focused().id, add);
        h.assert_sheet_focus();
        let (_, viewport, _) = h.app.schema_authoring.body_scroll.unwrap();
        assert!(viewport.contains_rect(h.focused().rect));
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert_eq!(
            h.app.schema_authoring.sessions[&target].values["label_id"],
            Some(PropertyValue::Str(String::new()))
        );
        h.press(Key::Escape, Modifiers::NONE);
        h.settle();
        assert!(!h.app.schema_authoring.open);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert!(h.app.schema_authoring.open);
        assert_eq!(
            h.app.schema_authoring.sessions[&target].values["label_id"],
            Some(PropertyValue::Str(String::new()))
        );
        focus_empty_field(&mut h, fields::input_id(&target, "label_id"));
        h.frame(vec![Event::Text("新增人物标签".into())]);
        h.settle();
        assert_eq!(
            h.app.schema_authoring.sessions[&target].values["label_id"],
            Some(PropertyValue::Str("新增人物标签".into()))
        );
        h.tab_to("预览字段修改", WidgetType::Button, true);
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        let plan = h.app.schema_authoring.sessions[&target]
            .preview
            .as_ref()
            .unwrap();
        assert!(!plan.has_errors);
        assert!(plan.runtime_changed);
        assert!(!plan
            .after
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SCH004"));
        assert_eq!(h.app.project.content_baseline(), baseline);
        h.tab_to("确认应用这次运行指纹变更", WidgetType::Checkbox, false);
        h.press(Key::Space, Modifiers::NONE);
        h.tab_to("应用到工程", WidgetType::Button, true);
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert_eq!(h.app.history.len(), 1);
        let applied = h.app.current_schema_authoring(&target).unwrap();
        assert!(applied.fields.iter().all(|field| field.present));
        assert_eq!(
            applied
                .fields
                .iter()
                .find(|field| field.id == "label_id")
                .unwrap()
                .value,
            Some(PropertyValue::Str("新增人物标签".into()))
        );
        assert!(h
            .app
            .project
            .document(&h.app.active_file)
            .unwrap()
            .contains("property custom = \"人物未知属性保留\""));
    }
}
