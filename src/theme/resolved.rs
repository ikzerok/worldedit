use super::{
    palette, AppearancePreferences, BodyFamily, Colors, Density, StylePreset, SyntaxPalette,
    ThemeMode,
};
use egui::{FontFamily, FontId, Vec2};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub control_height: f32,
    pub row_height: f32,
    pub section_gap: f32,
    pub panel_margin: f32,
    pub button_padding: Vec2,
    pub navigation_margin: f32,
    pub inspector_margin: f32,
    pub document_margin: Vec2,
    pub stage_margin: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shapes {
    pub control: u8,
    pub row: u8,
    pub document: u8,
    pub popup: u8,
    pub window: u8,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRoles {
    pub ui: FontId,
    pub body: FontId,
    pub source: FontId,
    pub heading: FontId,
    pub meta: FontId,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTheme {
    pub preferences: AppearancePreferences,
    pub effective_mode: ThemeMode,
    pub light: bool,
    pub colors: Colors,
    pub syntax: SyntaxPalette,
    pub metrics: Metrics,
    pub shapes: Shapes,
    pub type_roles: TypeRoles,
    pub focus_width: f32,
    pub revision: u64,
}

pub fn resolve(preferences: &AppearancePreferences, system: Option<egui::Theme>) -> ResolvedTheme {
    let mut p = *preferences;
    p.normalize();
    let effective_mode = p.palette.effective_mode(p.theme, system);
    let light = effective_mode == ThemeMode::Light;
    let (mut colors, mut syntax) = palette::palette(p.palette, light);
    palette::apply_accent(&mut colors, p.accent, light);
    if p.high_contrast {
        colors.secondary = colors.text;
        colors.control_border = colors.text;
        syntax.comment = colors.text;
    }
    let (height, gap, pad) = match p.density {
        Density::Compact => (26.0, 12.0, Vec2::new(8.0, 3.0)),
        Density::Standard => (30.0, 16.0, Vec2::new(10.0, 5.0)),
        Density::Spacious => (36.0, 22.0, Vec2::new(12.0, 8.0)),
    };
    let (shapes, margin, nav, inspector, document, stage) = match p.style {
        StylePreset::Studio => (
            Shapes {
                control: 4,
                row: 5,
                document: 0,
                popup: 8,
                window: 8,
            },
            16.0,
            10.0,
            14.0,
            Vec2::new(16.0, 16.0),
            0.0,
        ),
        StylePreset::Manuscript => (
            Shapes {
                control: 3,
                row: 3,
                document: 1,
                popup: 6,
                window: 6,
            },
            20.0,
            10.0,
            14.0,
            Vec2::new(32.0, 28.0),
            24.0,
        ),
        StylePreset::Technical => (
            Shapes {
                control: 2,
                row: 0,
                document: 0,
                popup: 2,
                window: 2,
            },
            8.0,
            8.0,
            10.0,
            Vec2::new(10.0, 8.0),
            0.0,
        ),
        StylePreset::Focus => (
            Shapes {
                control: 6,
                row: 6,
                document: 0,
                popup: 10,
                window: 10,
            },
            24.0,
            10.0,
            14.0,
            Vec2::new(20.0, 24.0),
            32.0,
        ),
        StylePreset::Ledger => (
            Shapes {
                control: 2,
                row: 2,
                document: 0,
                popup: 4,
                window: 4,
            },
            12.0,
            8.0,
            12.0,
            Vec2::new(16.0, 14.0),
            0.0,
        ),
    };
    let metrics = Metrics {
        control_height: height,
        row_height: height,
        section_gap: gap,
        panel_margin: margin,
        button_padding: pad,
        navigation_margin: nav,
        inspector_margin: inspector,
        document_margin: document,
        stage_margin: stage,
    };
    let body_family = match p.body_family {
        BodyFamily::Sans => FontFamily::Proportional,
        BodyFamily::Mono => FontFamily::Monospace,
    };
    let type_roles = TypeRoles {
        ui: FontId::proportional(super::BODY_SIZE),
        body: FontId::new(p.body_size, body_family),
        source: FontId::monospace(p.source_size),
        heading: FontId::proportional(super::HEADING_SIZE),
        meta: FontId::proportional(super::META_SIZE),
    };
    // Font bytes are installed separately once per Context. Increment when the bundled
    // font policy changes, so the appearance cache also has an explicit font revision.
    const FONT_REVISION: u8 = 2;
    let mut key = std::collections::hash_map::DefaultHasher::new();
    (
        p.theme,
        p.palette,
        p.style,
        p.density,
        p.accent,
        p.body_family,
        p.high_contrast,
        p.reduce_motion,
        light,
        FONT_REVISION,
    )
        .hash(&mut key);
    for value in [
        p.body_size,
        p.source_size,
        p.line_spacing,
        p.reading_width,
        p.ui_scale,
    ] {
        value.to_bits().hash(&mut key);
    }
    ResolvedTheme {
        preferences: p,
        effective_mode,
        light,
        colors,
        syntax,
        metrics,
        shapes,
        type_roles,
        focus_width: if p.high_contrast { 3.0 } else { 2.0 },
        revision: key.finish(),
    }
}
