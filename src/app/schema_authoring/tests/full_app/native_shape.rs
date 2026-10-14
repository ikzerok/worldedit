//! 已保存、多文件、双父窗的正常动画键盘轨迹；不注入焦点、选项或滚动状态。
use super::*;
use std::io::Write;

const WORLD: &str = r###"// Native v0.35 isolated authoring fixture. Preserve exact comments and unknown properties.
world globe as "世界验收"
  property label = "world02"
  property count = 0 // zero is present
  property public = false
  property note = ""
  property custom = "世界未知属性保留"
character lead as "人物验收"
  property label = "char02"
  property count = 0
  property public = false
  property note = ""
  property custom = "人物未知属性保留"
entity harbor kind place as "实体验收"
  property label = "entity03"
  property count = 0 // entity selected/unselected byte boundary
  property public = false
  property note = ""
  property category = "city"
  property custom = "未知\n原文" // do not rewrite this property
entity council kind organization as "议会引用候选"
entity remote kind place as "非组织引用排除"
character harbor as "跨种同ID人物"
relation_type knows as "知道"
  from entity
  to entity
relation_def bond type knows from entity harbor to entity council
  property label = "relation03"
  property count = 0
  property public = false
  property note = ""
  property custom = "关系未知属性保留"
event start
  原生验收起点。
  -> END
bind character lead to character_data
bind world globe to world_data
bind entity harbor to entity_data
bind relation bond to relation_data
"###;

const SCHEMA: &str = r###"// Open schemas preserve unknown instance properties.
schema world_data for world
  field label_id label text required
  field count_id count number required
  field public_id public boolean required
  field note_id note text required
schema character_data for character
  field label_id label text required
  field count_id count number required
  field public_id public boolean required
  field note_id note text required
schema entity_data for entity entity_type place
  field label_id label text required
  field count_id count number required
  field public_id public boolean required
  field note_id note text required
  field category_id category enum "city" "town"
  field owner_id owner ref entity entity_type organization
schema relation_data for relation
  field label_id label text required
  field count_id count number required
  field public_id public boolean required
  field note_id note text required
"###;

const BINDING: &str = r###"// Initially unbound; explicit binding only.
"###;

const MANIFEST: &str = r###"{
  "language_version": "1.13",
  "required_features": [
    "content.entities.v1",
    "content.object_refs.v1",
    "content.character_refs.v1",
    "content.relations.v1",
    "content.templates.v1"
  ],
  "schema_version": 1,
  "templates": {
    "project:shared_fields": ".world/templates/shared_fields.json"
  }
}
"###;

const TEMPLATE: &str = r###"{
  "applies_to": {
    "entity_type": "place",
    "kind": "entity"
  },
  "extensions": {
    "preserve": "native-v035"
  },
  "fields": [
    {
      "id": "tpl_count",
      "key": "count",
      "label": "模板计数（同key）",
      "required": false,
      "type": "number"
    },
    {
      "id": "tpl_note",
      "key": "note",
      "label": "模板备注（同key）",
      "required": false,
      "type": "text"
    },
    {
      "id": "tpl_extra",
      "key": "extra",
      "label": "模板独立字段",
      "required": false,
      "type": "text"
    }
  ],
  "id": "project:shared_fields",
  "schema_version": 1,
  "title": "同key模板验收"
}
"###;

fn saved_fixture() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "schema-native-shape-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("独占多文件磁盘夹具");
    for (name, text) in [
        ("world.wl", WORLD),
        ("schema.wl", SCHEMA),
        ("binding.wl", BINDING),
        (".world/project.json", MANIFEST),
        (".world/templates/shared_fields.json", TEMPLATE),
    ] {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    root
}

fn animation_settle(h: &mut FullApp) -> egui::FullOutput {
    // RawInput 的默认时间每帧推进 predicted_dt；一秒足够完成正常滚动动画。
    // 不把 reduce_motion 打开，也不直接写动画时间/scroll offset。
    for _ in 0..60 {
        h.frame(vec![]);
    }
    h.frame(vec![])
}

fn quick_navigation(h: &mut FullApp, query: &str, down: usize) {
    h.press(Key::P, Modifiers::COMMAND);
    h.frame(vec![Event::Text(query.into())]);
    animation_settle(h);
    assert!(h.app.command_palette.open);
    assert_eq!(h.focused().id, egui::Id::new("author-command-query"));
    for _ in 0..down {
        h.press(Key::ArrowDown, Modifiers::NONE);
    }
    h.press(Key::Enter, Modifiers::NONE);
    animation_settle(h);
    assert!(!h.app.command_palette.open);
}

fn record(h: &mut FullApp, phase: &str, info: &[egui::WidgetInfo]) {
    let response = h
        .ctx
        .memory(|m| m.focused())
        .and_then(|id| h.ctx.read_response(id));
    let (scroll_id, viewport, content) = h.app.schema_authoring.body_scroll.unwrap();
    let offset = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset;
    let line = format!(
        "{phase} input={:?} focus={:?} response={:?} info={info:?} body={viewport:?} content={content:?} offset={offset:?} animation={:?}",
        h.ctx.input(|i| (i.time, i.focused)),
        response.as_ref().map(|r| r.id),
        response.as_ref().map(|r| (r.layer_id, r.rect, r.interact_rect, r.enabled(), r.has_focus(), r.gained_focus())),
        h.ctx.style().scroll_animation
    );
    eprintln!("{line}");
    h.trace.push(line);
}

fn assert_tab_cycle(h: &mut FullApp, phase: &str) {
    let target = TargetRef::new("entity", "harbor");
    animation_settle(h);
    record(h, &format!("{phase}: actual opening focus"), &[]);
    // 起点必须来自真实 Button 的 FocusGained；大容器包含按钮文字不等于按钮。
    let mut preview_id = None;
    for step in 0..48 {
        let info = h.press(Key::Tab, Modifiers::NONE);
        let output = animation_settle(h);
        record(
            h,
            &format!("{phase}: locate preview Tab {}", step + 1),
            &info,
        );
        let response = h.focused();
        if info.last().is_some_and(|widget| {
            widget.typ == WidgetType::Button && widget.label.as_deref() == Some("预览字段修改")
        }) && h.response_visible(&response, &output, "预览字段修改")
        {
            preview_id = Some(response.id);
            break;
        }
    }
    let initial = h.focused();
    assert_eq!(
        preview_id,
        Some(initial.id),
        "无法经真实Tab到达预览按钮起点: response={initial:?}\n{}",
        h.trace.join("\n")
    );
    h.assert_sheet_focus();
    record(h, &format!("{phase}: initial preview"), &[]);
    let mut reached = [false; 3];
    let mut returned = false;
    for step in 1..=48 {
        let info = h.press(Key::Tab, Modifiers::NONE);
        record(h, &format!("{phase}: Tab {step} key release"), &info);
        let output = animation_settle(h);
        record(h, &format!("{phase}: Tab {step} settled"), &info);
        let Some(response) = h
            .ctx
            .memory(|m| m.focused())
            .and_then(|id| h.ctx.read_response(id))
        else {
            continue;
        };
        h.assert_sheet_focus();
        let field = if response.id == fields::input_id(&target, "note_id") {
            Some((0, "note"))
        } else if info.iter().any(|i| i.typ == WidgetType::ComboBox)
            && h.response_visible(&response, &output, "city")
        {
            Some((1, "enum"))
        } else if info.iter().any(|i| i.typ == WidgetType::ComboBox)
            && h.response_visible(&response, &output, "选择引用对象（未填写）")
        {
            Some((2, "ref"))
        } else {
            None
        };
        if let Some((index, label)) = field {
            let (_, viewport, _) = h.app.schema_authoring.body_scroll.unwrap();
            assert!(
                response.enabled()
                    && viewport.contains_rect(response.rect)
                    && response.interact_rect.contains_rect(response.rect)
                    && h.ctx.layer_id_at(response.rect.center()) == Some(response.layer_id),
                "{phase}: {label} 实际获得焦点但动画结束后不可见\n{}",
                h.trace.join("\n")
            );
            reached[index] = true;
        }
        if response.id == initial.id {
            eprintln!(
                "{phase}: Tab {step} returns to initial preview {:?}; reached={reached:?}",
                initial.id
            );
            returned = true;
            break;
        }
    }
    assert!(returned && reached.iter().all(|seen| *seen),
        "{phase}: 正常动画Tab闭环必须实际显出note/enum/ref: returned={returned}, reached={reached:?}\n{}",
        h.trace.join("\n"));
}

#[test]
fn schema_full_app_native_saved_shape_normal_animation_keyboard() {
    let root = saved_fixture();
    let mut h = FullApp::new();
    h.app.personal.settings.appearance.reduce_motion = false;
    h.app.load_project(root.clone());
    animation_settle(&mut h);
    assert!(h.app.io_error.is_none(), "{:?}", h.app.io_error);
    assert_eq!(h.app.project.root, root);
    assert!(!h.app.project.is_dirty());
    assert!(!h.app.personal.settings.appearance.reduce_motion);
    assert!(h.ctx.style().animation_time > 0.0);
    assert!(h.ctx.style().scroll_animation.duration.max > 0.0);
    let baseline = h.app.project.content_baseline();
    for (kind, id, schema, label) in [
        ("world", "globe", "world_data", "world02"),
        ("character", "lead", "character_data", "char02"),
        ("entity", "harbor", "entity_data", "entity03"),
        ("relation", "bond", "relation_data", "relation03"),
    ] {
        let projection = h
            .app
            .current_schema_authoring(&TargetRef::new(kind, id))
            .unwrap();
        assert_eq!(projection.binding.as_ref().unwrap().schema_id, schema);
        assert!(projection.diagnostics.is_empty());
        assert_eq!(
            projection.fields[0].value,
            Some(PropertyValue::Str(label.into()))
        );
    }
    let target = TargetRef::new("entity", "harbor");
    let projection = h.app.current_schema_authoring(&target).unwrap();
    assert_eq!(projection.fields.len(), 6);
    assert_eq!(
        projection.fields[5]
            .reference_candidates
            .iter()
            .map(|o| &o.target)
            .collect::<Vec<_>>(),
        vec![&TargetRef::new("entity", "council")]
    );
    quick_navigation(&mut h, "harbor", 1);
    assert_eq!(
        h.app.entity_editor.as_ref().unwrap().original.as_deref(),
        Some("harbor")
    );
    quick_navigation(&mut h, "bond", 0);
    assert!(h.app.entity_editor.is_some() && h.app.relation_editor.is_some());
    quick_navigation(&mut h, "harbor", 1);
    assert_eq!(
        h.focused().layer_id,
        egui::LayerId::new(egui::Order::Middle, egui::Id::new("entity-editor"))
    );
    h.tab_to("约束与属性…", WidgetType::Button, false);
    assert_eq!(
        h.focused().layer_id,
        egui::LayerId::new(egui::Order::Middle, egui::Id::new("entity-editor"))
    );
    h.press(Key::Enter, Modifiers::NONE);
    animation_settle(&mut h);
    assert!(h.app.schema_authoring.open);
    assert!(h.app.entity_editor.is_some() && h.app.relation_editor.is_some());
    assert_eq!(h.app.schema_authoring.active.as_ref(), Some(&target));
    assert_tab_cycle(&mut h, "saved bound values before preview");
    h.press(Key::Enter, Modifiers::NONE);
    animation_settle(&mut h);
    assert!(
        !h.app.schema_authoring.sessions[&target]
            .preview
            .as_ref()
            .unwrap()
            .has_errors
    );
    assert_tab_cycle(&mut h, "saved bound values after preview");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
}

#[test]
fn schema_full_app_native_ime_disabled_after_tab_preserves_focus() {
    let root = saved_fixture();
    let mut h = FullApp::new();
    h.app.personal.settings.appearance.reduce_motion = false;
    h.app.load_project(root);
    animation_settle(&mut h);
    quick_navigation(&mut h, "harbor", 1);
    quick_navigation(&mut h, "bond", 0);
    quick_navigation(&mut h, "harbor", 1);
    h.tab_to("约束与属性…", WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    animation_settle(&mut h);
    let target = TargetRef::new("entity", "harbor");
    let baseline = h.app.project.content_baseline();
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    h.tab_to("entity03", WidgetType::TextEdit, false);
    assert_eq!(h.focused().id, fields::input_id(&target, "label_id"));
    animation_settle(&mut h);

    // 原生实际次序：Tab down 离开 TextEdit，两个空帧，Tab up 同批 Ime Disabled。
    // Disabled 只是平台收起已失焦输入框的 IME，未发生 Preedit 或 Commit。
    let key = |pressed| Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let output = h.frame(vec![key(true)]);
    let info: Vec<_> = output
        .platform_output
        .events
        .iter()
        .filter_map(|event| match event {
            egui::output::OutputEvent::FocusGained(info) => Some(info.clone()),
            _ => None,
        })
        .collect();
    let remove = h.focused();
    assert!(info.iter().any(|info| info.typ == WidgetType::Button
        && info.label.as_deref() == Some("移除值（设为未填写）")));
    assert!(h.response_visible(&remove, &output, "移除值（设为未填写）"));
    assert!(remove.enabled() && remove.has_focus());
    record(&mut h, "native sequence Tab down: remove focused", &info);
    for _ in 0..2 {
        h.frame(vec![]);
    }
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(remove.id));
    h.frame(vec![key(false), Event::Ime(egui::ImeEvent::Disabled)]);
    record(&mut h, "native sequence Tab up + Ime Disabled", &[]);
    let actual = h.ctx.memory(|m| m.focused());
    let response = h.ctx.read_response(remove.id).unwrap();
    assert!(actual == Some(remove.id) && response.enabled() && response.has_focus(),
        "结束文本输入的原生Ime Disabled不得禁用刚获焦的Remove或丢失Tab位置: expected={:?}, actual={actual:?}, response={response:?}\n{}",
        remove.id, h.trace.join("\n"));
    animation_settle(&mut h);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(remove.id));
    // 平台启用输入法但尚无 Preedit，也不能锁住下一次正常 Tab。
    h.tab_to("entity03", WidgetType::TextEdit, false);
    assert_eq!(h.focused().id, fields::input_id(&target, "label_id"));
    h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
    let output = h.frame(vec![key(true)]);
    let after_enabled = h.focused();
    assert!(h.response_visible(&after_enabled, &output, "移除值（设为未填写）"));
    assert_ne!(after_enabled.id, fields::input_id(&target, "label_id"));
    h.frame(vec![key(false), Event::Ime(egui::ImeEvent::Disabled)]);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(after_enabled.id));
    // 后续仍由真实键盘从已识别的 preview 按钮完整走到 note、enum 和 ref。
    assert_tab_cycle(&mut h, "native lifecycle after leaving label");
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
}
