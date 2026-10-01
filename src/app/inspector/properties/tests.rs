use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use worldline_core::catalog::CatalogObject;

struct Form {
    ctx: egui::Context,
    catalog: Catalog,
    values: Vec<(String, PropertyValue)>,
    options: CompileOptions,
}
impl Form {
    fn new(options: CompileOptions) -> Self {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut catalog = Catalog::default();
        for (kind, file) in [("character", "people.wl"), ("entity", "places.wl")] {
            catalog.objects.push(CatalogObject {
                target: TargetRef::new(kind, "lin"),
                display: "林舟".into(),
                file: file.into(),
                line: 1,
            });
        }
        Self {
            ctx,
            catalog,
            values: vec![("captain".into(), PropertyValue::Str(String::new()))],
            options,
        }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    properties_with_references(ui, &mut self.values, &self.catalog, self.options);
                });
            },
        )
    }
    fn click(&mut self, text: &str) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
        let output = self.frame(Vec::new());
        let point = output
            .shapes
            .iter()
            .find_map(|shape| find(&shape.shape, text))
            .unwrap_or_else(|| panic!("找不到{text}"));
        self.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}
fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(value) if value.galley.job.text == text => {
            Some(value.pos + value.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(values) => values.iter().find_map(|shape| find(shape, text)),
        _ => None,
    }
}
fn has(output: &egui::FullOutput, text: &str) -> bool {
    output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, text).is_some())
}

#[test]
fn direct_character_typed_slot_uses_only_character_identity_candidates() {
    let mut form = Form::new(
        CompileOptions::v1_13()
            .with_object_refs(true)
            .with_character_refs(true),
    );
    form.click("文本");
    form.click("人物引用");
    assert_eq!(
        form.values[0].1,
        PropertyValue::Ref(TargetRef::new("character", ""))
    );
    form.click("请选择");
    let output = form.frame(Vec::new());
    assert!(has(&output, "林舟 · 人物:lin\npeople.wl:1"));
    assert!(!has(&output, "林舟 · 实体:lin\nplaces.wl:1"));
    form.click("林舟 · 人物:lin\npeople.wl:1");
    assert_eq!(
        form.values[0].1,
        PropertyValue::Ref(TargetRef::new("character", "lin"))
    );
}

#[test]
fn old_modes_hide_character_option_and_render_without_converting_existing_string() {
    for options in [
        CompileOptions::v1_12()
            .with_object_refs(true)
            .with_character_refs(true),
        CompileOptions::v1_13().with_object_refs(true),
    ] {
        let mut form = Form::new(options);
        form.values[0].1 = PropertyValue::Str("lin".into());
        form.click("文本");
        let output = form.frame(Vec::new());
        assert!(!has(&output, "人物引用"));
        assert!(has(&output, "实体引用"));
        assert!(has(&output, "关系引用"));
        assert_eq!(form.values[0].1, PropertyValue::Str("lin".into()));
    }
}

mod window;
