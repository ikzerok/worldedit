//! 从仍显示世界资料的完整 App，经真实快速导航进入实体/关系资料。
use super::*;

const SOURCE: &str = r#"schema world_data for world
  field note_id note text required
schema entity_data for entity entity_type place
  field note_id note text required
schema relation_data for relation
  field note_id note text required
world globe as "世界验收"
  property note = "世界初值"
character harbor as "跨种同ID人物"
entity harbor kind place as "实体验收"
  property note = "实体初值"
entity council kind organization as "议会"
relation_type knows as "知道"
relation_def bond type knows from entity harbor to entity council
  property note = "关系初值"
event start
  正文。
  -> END
"#;

fn quick_navigation(h: &mut FullApp, query: &str, down: usize) {
    h.press(Key::P, Modifiers::COMMAND);
    h.frame(vec![Event::Text(query.into())]);
    h.settle();
    assert!(h.app.command_palette.open, "{}", h.trace.join("\n"));
    assert_eq!(h.focused().id, egui::Id::new("author-command-query"));
    for _ in 0..down {
        h.press(Key::ArrowDown, Modifiers::NONE);
    }
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.command_palette.open, "{}", h.trace.join("\n"));
}

fn binding_and_field_preview(
    h: &mut FullApp,
    target: &TargetRef,
    schema: &str,
    old: &str,
    new: &str,
) {
    h.assert_sheet_focus();
    let baseline = h.app.project.content_baseline();
    h.tab_to("未绑定", WidgetType::ComboBox, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to(schema, WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to("预览所选绑定", WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.schema_authoring.active.as_ref(), Some(target));
    assert_eq!(
        h.app.schema_authoring.sessions[target]
            .projection
            .binding
            .as_ref()
            .unwrap()
            .schema_id,
        schema
    );
    h.tab_to(old, WidgetType::TextEdit, false);
    assert_eq!(h.focused().id, fields::input_id(target, "note_id"));
    h.press(Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Text(new.into())]);
    h.settle();
    h.tab_to("预览字段修改", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let plan = h.app.schema_authoring.sessions[target]
        .preview
        .as_ref()
        .unwrap();
    assert!(plan.changed && !plan.has_errors);
    assert_eq!(plan.after.target, *target);
    assert_eq!(
        plan.after.fields[0].value,
        Some(PropertyValue::Str(new.into()))
    );
    assert_eq!(h.app.project.content_baseline(), baseline);
}

fn saved_world_then_closed_sheet() -> FullApp {
    let mut h = FullApp::new();
    h.app
        .project
        .set_text(&h.app.active_file.clone(), SOURCE.into())
        .unwrap();
    h.app.project.save().unwrap();
    h.app.saved_location = true; // 仅夹具的已打开磁盘工程身份，后续保存走真实 Ctrl+S。
    h.app.recompile();
    quick_navigation(&mut h, "globe", 0);
    assert_eq!(h.app.tab, crate::app::Tab::World);
    let entry = h.tab_to("约束与属性…", WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let target = TargetRef::new("world", "globe");
    binding_and_field_preview(&mut h, &target, "world_data", "世界初值", "世界已应用");
    assert!(
        h.app.schema_authoring.sessions[&target]
            .preview
            .as_ref()
            .unwrap()
            .runtime_changed
    );
    h.tab_to("确认应用这次运行指纹变更", WidgetType::Checkbox, false);
    h.press(Key::Space, Modifiers::NONE);
    h.tab_to("应用到工程", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.project.is_dirty());
    h.press(Key::S, Modifiers::COMMAND);
    h.settle();
    assert!(!h.app.project.is_dirty());
    assert!(h.app.io_error.is_none());
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert!(h.app.world_editor.is_some());
    assert_eq!(h.app.tab, crate::app::Tab::World);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
    h
}

fn target_entry(h: &mut FullApp, window: &str) -> egui::Id {
    let expected = egui::LayerId::new(egui::Order::Middle, egui::Id::new(window));
    let initial = h
        .ctx
        .memory(|memory| memory.focused())
        .and_then(|id| h.ctx.read_response(id));
    let initially_in_window = initial
        .as_ref()
        .is_some_and(|response| response.layer_id == expected && response.enabled());
    let mut first_window_step = None;
    let mut clipped_entry = None;
    // 匹配目标窗，不能把下层世界页同名按钮当成入口成功。
    for (step, reverse) in std::iter::repeat_n(false, 72)
        .chain(std::iter::repeat_n(true, 32))
        .enumerate()
    {
        let info = h.press(
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        );
        let output = h.settle();
        let Some(response) = h
            .ctx
            .memory(|memory| memory.focused())
            .and_then(|id| h.ctx.read_response(id))
        else {
            continue;
        };
        if response.layer_id != expected {
            continue;
        }
        if first_window_step.is_none() {
            first_window_step = Some(step + 1);
            h.trace.push(format!(
                "首次进入 {window}: Tab步数={} reverse={reverse} response={:?} info={info:?}",
                step + 1,
                (
                    response.id,
                    response.layer_id,
                    response.rect,
                    response.enabled()
                )
            ));
        }
        if info.iter().any(|info| {
            info.typ == WidgetType::Button && info.label.as_deref() == Some("约束与属性…")
        }) {
            if h.response_visible(&response, &output, "约束与属性…") {
                assert!(initially_in_window,
                    "显式打开 {window} 后初始焦点仍在其他上下文: initial={:?}; first_window_step={first_window_step:?}\n{}",
                    initial.as_ref().map(|response| (response.id, response.layer_id, response.rect)), h.trace.join("\n"));
                return response.id;
            }
            clipped_entry = Some((
                response.id,
                response.rect,
                response.interact_rect,
                response.enabled(),
            ));
        }
    }
    panic!("目标资料窗入口键盘不可达: {window}; initial={:?}; first_window_step={first_window_step:?}; clipped_entry={clipped_entry:?}\n{}",
        initial.map(|response| (response.id, response.layer_id, response.rect)), h.trace.join("\n"));
}

#[test]
fn schema_full_app_world_to_entity_real_entry_reaches_binding_and_field_preview() {
    let mut h = saved_world_then_closed_sheet();
    quick_navigation(&mut h, "harbor", 1);
    assert_eq!(
        h.app.entity_editor.as_ref().unwrap().original.as_deref(),
        Some("harbor")
    );
    assert_eq!(
        h.app.catalog_target,
        Some(TargetRef::new("entity", "harbor"))
    );
    assert!(h.app.world_editor.is_some());
    let entry = target_entry(&mut h, "entity-editor");
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.schema_authoring.open);
    assert_eq!(h.app.schema_authoring.return_focus, Some(entry));
    binding_and_field_preview(
        &mut h,
        &TargetRef::new("entity", "harbor"),
        "entity_data",
        "实体初值",
        "实体键盘草稿",
    );
    assert!(h.app.entity_editor.is_some());
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
    assert!(h.app.world_editor.is_some());
}

#[test]
fn schema_full_app_world_to_relation_real_entry_reaches_binding_and_field_preview() {
    for entity_underneath in [false, true] {
        let mut h = saved_world_then_closed_sheet();
        if entity_underneath {
            quick_navigation(&mut h, "harbor", 1);
            assert_eq!(
                h.app.entity_editor.as_ref().unwrap().original.as_deref(),
                Some("harbor")
            );
        }
        quick_navigation(&mut h, "bond", 0);
        assert_eq!(
            h.app.relation_editor.as_ref().unwrap().original.as_deref(),
            Some("bond")
        );
        assert_eq!(
            h.app.catalog_target,
            Some(TargetRef::new("relation", "bond"))
        );
        assert!(h.app.world_editor.is_some());
        let entry = target_entry(&mut h, "relation-editor");
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert!(h.app.schema_authoring.open);
        assert_eq!(h.app.schema_authoring.return_focus, Some(entry));
        binding_and_field_preview(
            &mut h,
            &TargetRef::new("relation", "bond"),
            "relation_data",
            "关系初值",
            "关系键盘草稿",
        );
        assert!(h.app.relation_editor.is_some());
        assert_eq!(h.app.entity_editor.is_some(), entity_underneath);
        h.press(Key::Escape, Modifiers::NONE);
        h.settle();
        assert!(!h.app.schema_authoring.open);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
        assert_eq!(h.app.entity_editor.is_some(), entity_underneath);
        assert!(h.app.world_editor.is_some());
    }
}

#[test]
fn schema_full_app_world_to_new_entity_focus_preserves_commands_ime_and_cancel() {
    let mut h = saved_world_then_closed_sheet();
    let baseline = h.app.project.content_baseline();
    h.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    h.frame(vec![Event::Text("资料与状态".into())]);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.tab, crate::app::Tab::Catalog);
    h.tab_to("＋ 通用资料", WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let name_id = egui::Id::new("entity-editor-name");
    let response = h.focused();
    assert_eq!(response.id, name_id);
    assert_eq!(
        response.layer_id,
        egui::LayerId::new(egui::Order::Middle, egui::Id::new("entity-editor"))
    );
    assert!(response.enabled());
    assert!(h.ctx.screen_rect().contains_rect(response.rect));
    assert!(response.interact_rect.contains_rect(response.rect));
    assert!(h.app.entity_editor.as_ref().unwrap().original.is_none());
    assert!(!h.app.dirty_draft_names().contains(&"实体资料"));
    let kind_id = h.tab_to("地理与地点", WidgetType::ComboBox, false);
    h.press(Key::P, Modifiers::COMMAND);
    h.settle();
    let query_id = egui::Id::new("author-command-query");
    assert_eq!(h.focused().id, query_id);
    h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
    h.settle();
    assert_eq!(h.focused().id, query_id, "旧实体窗不能抢命令框组合焦点");
    assert!(h.app.entity_editor.is_some());
    assert!(!h.app.dirty_draft_names().contains(&"实体资料"));
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("候选".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.command_palette.open);
    assert_eq!(h.focused().id, kind_id);
    h.press(Key::Tab, Modifiers::SHIFT);
    h.settle();
    assert_eq!(h.focused().id, name_id);
    h.frame(vec![Event::Text("未应用的新实体".into())]);
    h.settle();
    assert!(h.app.dirty_draft_names().contains(&"实体资料"));
    h.tab_to("取消", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.entity_editor.is_none());
    assert!(h.app.world_editor.is_some());
    assert_eq!(h.app.project.content_baseline(), baseline);
}

#[test]
fn schema_full_app_world_to_reopened_entity_and_relation_focuses_latest_entry() {
    let mut h = saved_world_then_closed_sheet();
    let baseline = h.app.project.content_baseline();
    for (step, (query, down, window, guard)) in [
        ("harbor", 1, "entity-editor", "guard-entity"),
        ("bond", 0, "relation-editor", "guard-relation"),
        ("harbor", 1, "entity-editor", "guard-entity"),
        ("bond", 0, "relation-editor", "guard-relation"),
    ]
    .into_iter()
    .enumerate()
    {
        quick_navigation(&mut h, query, down);
        let response = h.focused();
        assert_eq!(
            response.layer_id,
            egui::LayerId::new(egui::Order::Middle, egui::Id::new(window)),
            "第 {step} 次显式打开 {window} 后必须立即进入该窗：{}",
            h.trace.join("\n")
        );
        assert!(response.enabled());
        assert!(h.ctx.screen_rect().contains_rect(response.rect));
        assert!(response.interact_rect.contains_rect(response.rect));
        assert!(h.app.edit_layer_is_top(guard));
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert!(!h.app.dirty_draft_names().contains(&"实体资料"));
        assert!(!h.app.dirty_draft_names().contains(&"语义关系"));
    }
    let entry = target_entry(&mut h, "relation-editor");
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    h.assert_sheet_focus();
    assert_eq!(h.app.schema_authoring.return_focus, Some(entry));
    assert!(h.app.entity_editor.is_some() && h.app.relation_editor.is_some());
}

#[test]
fn schema_full_app_world_to_source_selection_reopens_entity_above_relation() {
    let mut h = saved_world_then_closed_sheet();
    let baseline = h.app.project.content_baseline();
    quick_navigation(&mut h, "harbor", 1);
    quick_navigation(&mut h, "bond", 0);
    h.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    h.frame(vec![Event::Text("源文件".into())]);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.tab, crate::app::Tab::Edit);
    // 保留两个旧窗，用真实标题栏拖动露出源码，不能注入焦点或隐藏旧表单。
    drag_window_right(&mut h, "relation-editor", "编辑独立关系");
    quick_navigation(&mut h, "harbor", 1);
    assert_eq!(
        h.focused().layer_id,
        egui::LayerId::new(egui::Order::Middle, egui::Id::new("entity-editor"))
    );
    drag_window_right(&mut h, "entity-editor", "编辑通用资料");
    quick_navigation(&mut h, "bond", 0);
    assert_eq!(
        h.focused().layer_id,
        egui::LayerId::new(egui::Order::Middle, egui::Id::new("relation-editor"))
    );
    assert!(h.app.edit_layer_is_top("guard-relation"));
    let source_id = egui::Id::new(("source", &h.app.active_file));
    let source = h.ctx.read_response(source_id).unwrap();
    assert!(source.enabled(), "旧资料窗仍须保留非模态的源码交互");
    let rect = source.interact_rect.shrink(2.0);
    let point = (0..=20)
        .flat_map(|x| {
            (0..=20).map(move |y| {
                rect.min
                    + vec2(
                        rect.width() * x as f32 / 20.0,
                        rect.height() * y as f32 / 20.0,
                    )
            })
        })
        .find(|point| h.ctx.layer_id_at(*point) == Some(source.layer_id))
        .unwrap_or_else(|| panic!(
            "没有未遮挡源码点: source={:?}, interact={:?}, layer={:?}, center_layer={:?}, entity={:?}, relation={:?}",
            source.rect, source.interact_rect, source.layer_id, h.ctx.layer_id_at(rect.center()),
            h.ctx.memory(|memory| memory.area_rect(egui::Id::new("entity-editor"))),
            h.ctx.memory(|memory| memory.area_rect(egui::Id::new("relation-editor"))),
        ));
    for pressed in [true, false] {
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    h.settle();
    assert_eq!(h.focused().id, source_id);
    let text = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    let selected = "正文。";
    let end = text.find(selected).unwrap() + selected.len();
    h.press(Key::End, Modifiers::CTRL | Modifiers::COMMAND);
    let cursor = egui::TextEdit::load_state(&h.ctx, source_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(cursor.primary.index, text.chars().count());
    for _ in 0..text[end..].chars().count() {
        h.press(Key::ArrowLeft, Modifiers::NONE);
    }
    for _ in 0..selected.chars().count() {
        h.press(Key::ArrowLeft, Modifiers::SHIFT);
    }
    let cursor = egui::TextEdit::load_state(&h.ctx, source_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let selected_text: String = text
        .chars()
        .skip(cursor.primary.index.min(cursor.secondary.index))
        .take(cursor.primary.index.abs_diff(cursor.secondary.index))
        .collect();
    assert_eq!(selected_text, selected);
    h.press(Key::Enter, Modifiers::COMMAND);
    h.settle();
    assert_eq!(h.focused().id, egui::Id::new("entity-editor-name"));
    assert!(h.app.edit_layer_is_top("guard-entity"));
    let form = h.app.entity_editor.as_ref().unwrap();
    assert!(form.original.is_none());
    assert_eq!(form.draft.display, selected);
    assert_eq!(
        form.source_selection.as_ref().unwrap().expected_text,
        selected
    );
    assert!(h.app.relation_editor.is_some());
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.tab_to("取消", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.entity_editor.is_none());
    assert!(h.app.relation_editor.is_some());
    assert_eq!(h.app.project.content_baseline(), baseline);
    let returned = h.focused();
    assert_eq!(returned.id, source_id);
    assert!(returned.enabled());
    assert!(returned.interact_rect.contains(point));
    assert!(h.ctx.screen_rect().contains_rect(returned.interact_rect));
    assert_eq!(h.ctx.layer_id_at(point), Some(returned.layer_id));
}

fn drag_window_right(h: &mut FullApp, window: &str, title: &str) {
    let id = egui::Id::new(window);
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    let before = h.ctx.memory(|memory| memory.area_rect(id)).unwrap();
    if before.right() >= h.ctx.screen_rect().right() - 1.0 {
        return; // 已贴右边界时无需再次拖动；只移动仍挡住左侧源码的另一窗。
    }
    let output = h.settle();
    let title_rect = FullApp::visible_text(&output, title)
        .unwrap_or_else(|| panic!("标题未在clip内: {title}; window={before:?}"));
    let start = title_rect.center();
    assert_eq!(h.ctx.layer_id_at(start), Some(layer));
    let destination = pos2(h.ctx.screen_rect().right() - 24.0, start.y);
    h.frame(vec![Event::PointerMoved(start)]);
    h.frame(vec![Event::PointerButton {
        pos: start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    h.frame(vec![Event::PointerMoved(destination)]);
    h.frame(vec![Event::PointerButton {
        pos: destination,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    h.settle();
    let after = h.ctx.memory(|memory| memory.area_rect(id)).unwrap();
    assert!(
        after.left() > before.left() + 5.0,
        "{window}真实拖动必须改变窗体位置: {before:?} -> {after:?}"
    );
}
