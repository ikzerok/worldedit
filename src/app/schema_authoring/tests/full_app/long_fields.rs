//! 六字段长表真实键盘回归；与 native 实体形状相同，不注入 focus 或 scroll。
use super::*;

const SOURCE: &str = r#"schema entity_data for entity entity_type place
  field label_id label text required
  field count_id count number required
  field public_id public boolean required
  field note_id note text required
  field category_id category enum "city" "town"
  field owner_id owner ref entity entity_type organization
entity harbor kind place as "实体验收"
  property count = 0
  property public = false
  property note = ""
  property category = "city"
  property custom = "未知属性保留"
entity council kind organization as "议会引用候选"
entity remote kind place as "非组织引用排除"
character harbor as "跨种同ID人物"
event start
  正文。
  -> END
"#;

fn observe(h: &mut FullApp, phase: &str, info: &[egui::WidgetInfo]) {
    let response = h
        .ctx
        .memory(|memory| memory.focused())
        .and_then(|id| h.ctx.read_response(id));
    let scroll = h
        .app
        .schema_authoring
        .body_scroll
        .map(|(id, rect, content)| {
            (
                egui::scroll_area::State::load(&h.ctx, id).unwrap().offset,
                rect,
                content,
            )
        });
    h.trace.push(format!(
        "{phase} focus={:?} widget={:?} scroll={scroll:?} info={info:?}",
        response.as_ref().map(|r| r.id),
        response
            .as_ref()
            .map(|r| (r.layer_id, r.rect, r.interact_rect, r.enabled()))
    ));
}

fn tab_to(
    h: &mut FullApp,
    label: &str,
    typ: WidgetType,
    id: Option<egui::Id>,
    reverse: bool,
) -> egui::Id {
    for step in 0..100 {
        let info = h.press(
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        );
        let output = h.settle();
        observe(
            h,
            &format!("Tab {step} -> {label}, reverse={reverse}"),
            &info,
        );
        let Some(response) = h
            .ctx
            .memory(|memory| memory.focused())
            .and_then(|id| h.ctx.read_response(id))
        else {
            continue;
        };
        if h.app.schema_authoring.open && !egui::Popup::is_any_open(&h.ctx) {
            h.assert_sheet_focus();
        }
        let matches = id.map_or_else(
            || {
                info.iter().any(|info| {
                    info.typ == typ
                        && (info.label.as_deref() == Some(label) || typ == WidgetType::ComboBox)
                })
            },
            |id| response.id == id,
        );
        if matches {
            let visible = if id.is_some() {
                let (_, viewport, _) = h.app.schema_authoring.body_scroll.unwrap();
                response.enabled()
                    && viewport.contains_rect(response.rect)
                    && response.interact_rect.contains_rect(response.rect)
                    && h.ctx.layer_id_at(response.rect.center()) == Some(response.layer_id)
            } else {
                h.response_visible(&response, &output, label)
            };
            if visible {
                return response.id;
            }
        }
    }
    panic!(
        "长表控件 {label} 未通过真实焦点/clip断言：{}",
        h.trace.join("\n")
    );
}

fn run_long_fields(zoom_twice: bool) {
    let mut h = FullApp::new();
    h.app.project.set_authoring_document(
        &h.app.project.root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1","content.object_refs.v1","content.character_refs.v1","content.relations.v1"],"maps":{},"graph_views":{}}"#.to_vec()
    ).unwrap();
    h.app
        .project
        .set_text(&h.app.active_file.clone(), SOURCE.into())
        .unwrap();
    h.app.recompile();
    assert!(h.app.project.compile_options().object_refs);
    let baseline = h.app.project.content_baseline();
    h.press(Key::P, Modifiers::COMMAND);
    h.frame(vec![Event::Text("harbor".into())]);
    h.settle();
    h.press(Key::ArrowDown, Modifiers::NONE);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(
        h.app.catalog_target,
        Some(TargetRef::new("entity", "harbor"))
    );
    tab_to(&mut h, "约束与属性…", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    if zoom_twice {
        for _ in 0..10 {
            h.press(Key::Equals, Modifiers::COMMAND);
        }
        h.settle();
        assert_eq!(h.ctx.pixels_per_point(), 2.0);
    }
    h.assert_frame_visible();
    tab_to(&mut h, "未绑定", WidgetType::ComboBox, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    tab_to(&mut h, "entity_data", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    tab_to(&mut h, "预览所选绑定", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let target = TargetRef::new("entity", "harbor");
    assert_eq!(
        h.app.schema_authoring.sessions[&target]
            .projection
            .fields
            .len(),
        6
    );
    tab_to(&mut h, "添加此字段", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    tab_to(
        &mut h,
        "label",
        WidgetType::TextEdit,
        Some(fields::input_id(&target, "label_id")),
        false,
    );
    h.frame(vec![Event::Text("entity03".into())]);
    h.settle();
    tab_to(&mut h, "预览字段修改", WidgetType::Button, None, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let plan = h.app.schema_authoring.sessions[&target]
        .preview
        .as_ref()
        .unwrap();
    assert!(!plan.has_errors);
    assert!(plan.after.diagnostics.is_empty());

    // label/count/public 后必须能继续进入原本在滚动区下方的 note、enum、ref。
    tab_to(
        &mut h,
        "note",
        WidgetType::TextEdit,
        Some(fields::input_id(&target, "note_id")),
        false,
    );
    h.frame(vec![Event::Text("键盘备注".into())]);
    h.settle();
    let category = tab_to(&mut h, "city", WidgetType::ComboBox, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    tab_to(&mut h, "town", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!egui::ComboBox::is_open(&h.ctx, category));
    let owner = tab_to(
        &mut h,
        "选择引用对象（未填写）",
        WidgetType::ComboBox,
        None,
        false,
    );
    let candidates = &h.app.schema_authoring.sessions[&target]
        .projection
        .fields
        .iter()
        .find(|field| field.id == "owner_id")
        .unwrap()
        .reference_candidates;
    assert_eq!(
        candidates
            .iter()
            .map(|object| &object.target)
            .collect::<Vec<_>>(),
        vec![&TargetRef::new("entity", "council")]
    );
    h.press(Key::Enter, Modifiers::NONE);
    let output = h.settle();
    assert!(egui::ComboBox::is_open(&h.ctx, owner));
    assert!(
        FullApp::visible_text(&output, "议会引用候选 · entity:council").is_some(),
        "core提供候选后弹层应实际绘制可见候选行: {}",
        h.trace.join("\n")
    );
    tab_to(
        &mut h,
        "议会引用候选 · entity:council",
        WidgetType::Button,
        None,
        false,
    );
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!egui::ComboBox::is_open(&h.ctx, owner));
    let values = &h.app.schema_authoring.sessions[&target].values;
    assert_eq!(
        values["label_id"],
        Some(PropertyValue::Str("entity03".into()))
    );
    assert_eq!(values["count_id"], Some(PropertyValue::Num(0.0)));
    assert_eq!(values["public_id"], Some(PropertyValue::Bool(false)));
    assert_eq!(
        values["note_id"],
        Some(PropertyValue::Str("键盘备注".into()))
    );
    assert_eq!(
        values["category_id"],
        Some(PropertyValue::Str("town".into()))
    );
    assert_eq!(
        values["owner_id"],
        Some(PropertyValue::Ref(TargetRef::new("entity", "council")))
    );
    tab_to(&mut h, "预览字段修改", WidgetType::Button, None, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(
        !h.app.schema_authoring.sessions[&target]
            .preview
            .as_ref()
            .unwrap()
            .has_errors
    );
    tab_to(&mut h, "应用到工程", WidgetType::Button, None, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.history.len(), 1);
    let applied = h.app.current_schema_authoring(&target).unwrap();
    assert!(applied.fields.iter().all(|field| field.present));
    assert_eq!(
        applied
            .fields
            .iter()
            .find(|field| field.id == "owner_id")
            .unwrap()
            .value,
        Some(PropertyValue::Ref(TargetRef::new("entity", "council")))
    );
}

#[test]
fn schema_full_app_long_fields_keyboard_100pct() {
    run_long_fields(false);
}
#[test]
fn schema_full_app_long_fields_keyboard_200pct() {
    run_long_fields(true);
}
