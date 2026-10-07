//! egui真实帧/按键回归；不代替物理输入法与原生平台验收。
use super::*;
use egui::{Event, Key, Modifiers, Pos2, Rect};

struct Harness {
    ctx: egui::Context,
    catalog: Catalog,
    current: Option<TargetRef>,
    filter: ObjectSearchFilter,
    id: egui::Id,
    button: egui::Id,
}
impl Harness {
    fn new(count: usize) -> Self {
        Self {
            ctx: egui::Context::default(),
            catalog: Catalog {
                objects: (0..count)
                    .map(|index| CatalogObject {
                        target: TargetRef::new("entity", &format!("item_{index:04}")),
                        display: "同名对象".into(),
                        file: "世界/深层.wl".into(),
                        line: index as u32 + 1,
                    })
                    .collect(),
                ..Default::default()
            },
            current: Some(TargetRef::new("entity", "item_0000")),
            filter: filter(&[], None),
            id: egui::Id::NULL,
            button: egui::Id::NULL,
        }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        ctx.style_mut(|style| style.animation_time = 0.0);
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.id = ui.make_persistent_id("fixture-picker");
                    self.button = picker(
                        ui,
                        "fixture-picker",
                        "关联对象",
                        &mut self.current,
                        &self.catalog,
                        &self.filter,
                        false,
                    )
                    .1;
                });
            },
        )
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..6 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    fn open(&mut self) {
        let output = self.settle();
        let rect = text_rect(&output, "同名对象 · entity:item_0000").expect("实际绘制引用按钮");
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(rect.center()),
                Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
        self.settle();
        assert!(egui::ComboBox::is_open(&self.ctx, self.button));
    }
    fn key(&mut self, key: Key) {
        self.frame(vec![key_event(key)]);
    }
    fn state(&self) -> PickerState {
        self.ctx
            .data(|data| data.get_temp::<PickerState>(self.id))
            .unwrap()
    }
}
fn key_event(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}
fn text_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    fn rect(shape: &egui::Shape, label: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| rect(shape, label)),
            _ => None,
        }
    }
    output.shapes.iter().find_map(|shape| {
        rect(&shape.shape, label).filter(|rect| shape.clip_rect.contains_rect(*rect))
    })
}

#[test]
fn real_page_keys_reach_fifteen_hundredth_identity_without_changing_original_until_enter() {
    let mut h = Harness::new(1500);
    let original = h.current.clone();
    h.open();
    for _ in 0..74 {
        h.key(Key::PageDown);
        h.frame(vec![]);
    }
    assert_eq!(h.state().page.options.offset, 1480);
    assert_eq!(h.state().page.result.unwrap().unwrap().total, 1500);
    for _ in 0..19 {
        h.key(Key::ArrowDown);
    }
    let output = h.settle();
    assert!(text_rect(&output, &candidate_label(&h.catalog.objects[1499])).is_some());
    assert_eq!(h.current, original);
    h.key(Key::Enter);
    assert_eq!(h.current, Some(TargetRef::new("entity", "item_1499")));
    assert!(!egui::ComboBox::is_open(&h.ctx, h.button));
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.button));
}

#[test]
fn ime_empty_error_and_escape_preserve_original_and_return_button_focus() {
    let mut h = Harness::new(50);
    let original = h.current.clone();
    h.open();
    assert_eq!(
        h.ctx.memory(|memory| memory.focused()),
        Some(h.id.with("query")),
        "弹出选择器必须把焦点放在搜索输入"
    );
    h.key(Key::ArrowDown);
    assert_eq!(
        h.ctx.memory(|memory| memory.focused()),
        Some(h.id.with("query")),
        "箭头导航候选应保持搜索输入焦点"
    );
    let selected = h.state().selected;
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
    ] {
        let query_before = h.state().query;
        h.frame(vec![
            Event::Ime(event),
            key_event(Key::ArrowDown),
            key_event(Key::PageDown),
            key_event(Key::Enter),
            key_event(Key::Escape),
        ]);
        assert!(egui::ComboBox::is_open(&h.ctx, h.button));
        if h.state().query == query_before {
            assert_eq!(h.state().selected, selected);
        }
        assert_eq!(h.state().page.options.offset, 0);
        assert_eq!(h.current, original);
    }
    assert_eq!(
        h.state().query,
        "中",
        "preedit必须进入真实搜索输入；focus={:?}",
        h.ctx.memory(|m| m.focused())
    );
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Commit("无匹配".into())),
        key_event(Key::Enter),
    ]);
    h.settle();
    h.key(Key::Enter);
    assert_eq!(h.current, original);
    assert_eq!(h.state().page.result.unwrap().unwrap().total, 0);
    h.key(Key::Escape);
    assert_eq!(h.current, original);
    assert!(!egui::ComboBox::is_open(&h.ctx, h.button));
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.button));
    h.open();
    let mut state = h.state();
    state.query.clear();
    state.page.options.max_candidates = 1;
    h.ctx.data_mut(|data| data.insert_temp(h.id, state));
    h.settle();
    h.key(Key::Enter);
    assert!(h.state().page.result.unwrap().is_err());
    assert_eq!(h.current, original);
}

#[test]
fn source_change_clears_old_page_and_rejects_same_frame_enter() {
    let mut h = Harness::new(50);
    h.open();
    h.key(Key::PageDown);
    h.settle();
    let original = h.current.clone();
    assert_eq!(h.state().page.options.offset, 20);
    h.catalog.objects[0].file = "已移动/来源.wl".into();
    h.key(Key::Enter);
    assert_eq!(h.current, original);
    assert_eq!(h.state().page.options.offset, 0);
    assert!(egui::ComboBox::is_open(&h.ctx, h.button));
    h.key(Key::Escape);
    assert_eq!(h.current, original);
}

#[test]
fn pointer_down_then_refresh_or_page_change_then_up_cannot_confirm_a_replaced_row() {
    for turn_page in [false, true] {
        let mut h = Harness::new(50);
        h.open();
        let original = h.current.clone();
        let output = h.settle();
        let point = text_rect(&output, &candidate_label(&h.catalog.objects[1]))
            .unwrap()
            .center();
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        if turn_page {
            // Simulate the authorized page control state change while a mouse press remains armed.
            let mut state = h.state();
            state.page.options.offset = 20;
            h.ctx.data_mut(|data| data.insert_temp(h.id, state));
        } else {
            // Same full target/file/line, different projection generation: a stable target ID alone is insufficient.
            h.catalog.objects[1].display = "更新名称".into();
        }
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        assert_eq!(
            h.current, original,
            "不得在按下与抬起之间把过期候选变成新选择"
        );
        assert!(egui::ComboBox::is_open(&h.ctx, h.button));
    }
}
