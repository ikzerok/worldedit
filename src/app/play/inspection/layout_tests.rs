//! 完整App→普通试玩→真实检查窗口的合成egui矩阵；不冒充物理显示器或IME。
use super::{setup, unchanged};
use crate::app::{Tab, WorldeditApp};
use crate::theme::{
    AppearancePreferences, Density, PaletteId, PaletteModeSupport, StylePreset, ThemeMode,
};
use egui::{Event, Pos2, Rect, Vec2};
use worldline_runtime::{InspectionChange, Value};
#[path = "layout_tests/source_readability.rs"]
mod source_readability;

const STYLES: [StylePreset; 5] = [
    StylePreset::Studio,
    StylePreset::Manuscript,
    StylePreset::Technical,
    StylePreset::Focus,
    StylePreset::Ledger,
];
const WINDOW_ID: &str = "live-state-inspection";

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    size: Vec2,
}
impl Harness {
    fn new(appearance: AppearancePreferences, size: Vec2) -> Self {
        let (ctx, mut app) = setup();
        let story = app.play.as_mut().unwrap().story.as_mut().unwrap();
        story.choose(0).unwrap();
        story.continue_story().unwrap();
        app.personal.pending_restore = false;
        app.personal.settings.appearance = appearance;
        app.replay_debugger.inspection.query.text = "count".into();
        app.replay_debugger.inspection.show(&ctx);
        Self { ctx, app, size }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                // 不是单独调用检查器或颜色函数：经过实际App的外观安装、面板与普通试玩分发。
                eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
            },
        )
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
        self.frame(Vec::new())
    }
    fn window(&self) -> Rect {
        self.ctx
            .memory(|memory| memory.area_rect(egui::Id::new(WINDOW_ID)))
            .expect("实际状态检查Window必须已创建")
    }
    fn visible(&self, output: &egui::FullOutput, label: &str) -> Option<Rect> {
        let window = self
            .window()
            .intersect(Rect::from_min_size(Pos2::ZERO, self.size));
        output.shapes.iter().find_map(|clipped| {
            text_rect(&clipped.shape, label, clipped.clip_rect.intersect(window))
        })
    }
    fn assert_story_page(&self) {
        assert_eq!(self.app.tab, Tab::Play);
        assert!(self.app.replay_debugger.inspection.open);
        let story = self.app.play.as_ref().unwrap().story.as_ref().unwrap();
        assert!(story.is_paused());
        assert_eq!(story.replay_trace().steps.len(), 1);
        let page = self.app.replay_debugger.inspection.page.as_ref().unwrap();
        assert_eq!(page.first_observation, Some(1));
        assert_eq!(page.previous_observation, Some(1));
        assert_eq!(page.current_observation, Some(2));
        assert_eq!(page.total_matches, 1);
        let item = &page.items[0];
        assert_eq!(item.key.name, "count");
        assert_eq!(item.first.value, Some(Value::Num(0.0)));
        assert_eq!(item.previous.value, Some(Value::Num(0.0)));
        assert_eq!(item.current.value, Some(Value::Num(2.0)));
        assert_eq!(item.previous_change, InspectionChange::Changed);
    }
    /// 只发送真实合成滚轮输入，检查clip及视口；不以离屏galley声称可见。
    fn reveal(&mut self, label: &str) -> Rect {
        let mut output = self.frame(Vec::new());
        if let Some(rect) = self.visible(&output, label) {
            return rect;
        }
        for direction in [-1.0, 1.0] {
            for attempt in 0..48 {
                let window = self
                    .window()
                    .intersect(Rect::from_min_size(Pos2::ZERO, self.size));
                let row_anchor = ["全局变量 · count", "当前", "上次", "首次", "数值", "有变化"]
                    .iter()
                    .find_map(|text| self.visible(&output, text))
                    .map(|rect| rect.center());
                let point = match attempt % 3 {
                    0 => row_anchor.unwrap_or(window.center()),
                    1 => Pos2::new(window.center().x, window.top() + window.height() * 0.45),
                    _ => Pos2::new(window.center().x, window.bottom() - 25.0),
                };
                self.frame(vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, 72.0 * direction),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                output = self.frame(Vec::new());
                if let Some(rect) = self.visible(&output, label) {
                    return rect;
                }
            }
        }
        panic!(
            "检查窗口滚动后控件仍不可见：{label}；size={:?}, appearance={:?}, window={:?}",
            self.size,
            self.app.personal.appearance(),
            self.window()
        );
    }
    fn click(&mut self, label: &str) {
        let point = self.reveal(label).center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
    }
}
fn text_rect(shape: &egui::Shape, label: &str, clip: Rect) -> Option<Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            clip.contains_rect(rect).then_some(rect)
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| text_rect(shape, label, clip)),
        _ => None,
    }
}

#[test]
fn live_inspector_full_app_renders_all_eighty_five_palette_style_combinations() {
    let mut combinations = 0;
    for palette in PaletteId::ALL {
        let modes = match palette.mode_support() {
            PaletteModeSupport::Both => vec![ThemeMode::Light, ThemeMode::Dark],
            PaletteModeSupport::LightOnly => vec![ThemeMode::Light],
            PaletteModeSupport::DarkOnly => vec![ThemeMode::Dark],
        };
        for mode in modes {
            for style in STYLES {
                let mut h = Harness::new(
                    AppearancePreferences {
                        palette,
                        theme: mode,
                        style,
                        reduce_motion: true,
                        ..Default::default()
                    },
                    Vec2::new(1280.0, 800.0),
                );
                let before = unchanged(&h.app);
                let output = h.settle();
                assert!(!output.shapes.is_empty());
                h.assert_story_page();
                for label in [
                    "状态检查 · 当前真实试玩",
                    "返回试玩与选择",
                    "状态与变化",
                    "实际条件 / 动作证据",
                    "全局",
                    "调用局部",
                    "状态集",
                    "只看变化",
                    "上一观测",
                    "首次观测",
                    "全局变量 · count",
                    "有变化",
                    "首次",
                    "上次",
                    "当前",
                    "定位变量声明",
                ] {
                    assert!(
                        h.visible(&output, label).is_some(),
                        "{palette:?}/{mode:?}/{style:?}: {label}"
                    );
                }
                let theme = crate::theme::resolved(&h.ctx);
                assert_eq!(theme.preferences.palette, palette);
                assert_eq!(theme.preferences.style, style);
                assert_eq!(theme.effective_mode, mode);
                assert_eq!(unchanged(&h.app), before);
                combinations += 1;
            }
        }
    }
    assert_eq!(combinations, 85);
}

#[test]
fn live_inspector_full_app_narrow_and_two_hundred_percent_keep_controls_reachable() {
    for (style, palette, mode, density) in [
        (
            StylePreset::Studio,
            PaletteId::Mist,
            ThemeMode::Light,
            Density::Standard,
        ),
        (
            StylePreset::Manuscript,
            PaletteId::Vellum,
            ThemeMode::Light,
            Density::Spacious,
        ),
        (
            StylePreset::Technical,
            PaletteId::Terminal,
            ThemeMode::Dark,
            Density::Compact,
        ),
    ] {
        // 与已有外观验收一致：400×300逻辑点、200%对应800×600像素工作面积。
        for (size, scale) in [
            (Vec2::new(800.0, 600.0), 1.0),
            (Vec2::new(400.0, 300.0), 2.0),
        ] {
            let mut h = Harness::new(
                AppearancePreferences {
                    palette,
                    theme: mode,
                    style,
                    density,
                    ui_scale: scale,
                    reduce_motion: true,
                    ..Default::default()
                },
                size,
            );
            let before = unchanged(&h.app);
            let output = h.settle();
            assert!(!output.shapes.is_empty());
            h.assert_story_page();
            assert_eq!(h.ctx.zoom_factor(), scale);
            for label in ["状态检查 · 当前真实试玩", "返回试玩与选择", "状态与变化"]
            {
                assert!(
                    h.visible(&output, label).is_some(),
                    "{size:?}/{scale}/{style:?}: {label}"
                );
            }
            for label in [
                "实际条件 / 动作证据",
                "只看变化",
                "首次观测",
                "全局变量 · count",
                "有变化",
                "当前",
                "定位变量声明",
            ] {
                h.reveal(label);
            }
            h.assert_story_page();
            assert_eq!(unchanged(&h.app), before);
            h.click("返回试玩与选择");
            h.frame(Vec::new());
            assert!(!h.app.replay_debugger.inspection.open);
            assert_eq!(unchanged(&h.app), before);
        }
    }
}
