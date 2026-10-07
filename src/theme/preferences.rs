//! 仅设备外观：没有工程内容、文件路径或布局选择。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaletteId {
    #[default]
    Mist,
    Plum,
    Copper,
    SeaSalt,
    Graphite,
    Terminal,
    Neon,
    Vellum,
    Sakura,
    Monochrome,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteModeSupport {
    Both,
    LightOnly,
    DarkOnly,
}
impl PaletteModeSupport {
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "浅色与深色",
            Self::LightOnly => "仅浅色",
            Self::DarkOnly => "仅深色",
        }
    }
    pub fn notice(self) -> Option<&'static str> {
        match self {
            Self::Both => None,
            Self::LightOnly => Some(
                "本配色固定浅色；已保留原模式偏好，切回双模式配色后恢复。跟随系统在本配色下不改变明暗。",
            ),
            Self::DarkOnly => Some(
                "本配色固定深色；已保留原模式偏好，切回双模式配色后恢复。跟随系统在本配色下不改变明暗。",
            ),
        }
    }
}
impl PaletteId {
    pub const ALL: [Self; 10] = [
        Self::Mist,
        Self::Plum,
        Self::Copper,
        Self::SeaSalt,
        Self::Graphite,
        Self::Terminal,
        Self::Neon,
        Self::Vellum,
        Self::Sakura,
        Self::Monochrome,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Mist => "雾松",
            Self::Plum => "梅墨",
            Self::Copper => "纸铜",
            Self::SeaSalt => "海盐",
            Self::Graphite => "石墨",
            Self::Terminal => "终端绿",
            Self::Neon => "霓虹夜景",
            Self::Vellum => "羊皮墨",
            Self::Sakura => "樱花",
            Self::Monochrome => "印刷黑白",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Mist => "自然灰绿与安静的连续正文",
            Self::Plum => "梅紫墨色与柔和的文稿层级",
            Self::Copper => "暖纸色与鲜明铜橙强调",
            Self::SeaSalt => "冷白海盐与靛蓝强调",
            Self::Graphite => "中性石墨与冰青强调",
            Self::Terminal => "绿黑底、磷绿文字；不改变字体，不添加扫描线或闪光",
            Self::Neon => "蓝紫夜色、亮粉强调与冰青焦点；没有发光或背景纹理",
            Self::Vellum => "灰橄榄纸面与历史墨色；无纹理，区别于纸铜的暖橙",
            Self::Sakura => "淡粉纸面或深樱褐底，配莓红与樱粉强调",
            Self::Monochrome => "黑白主体与灰阶语法；错误、警告和状态保留必要提示色",
        }
    }
    pub fn mode_support(self) -> PaletteModeSupport {
        match self {
            Self::Terminal | Self::Neon => PaletteModeSupport::DarkOnly,
            Self::Vellum => PaletteModeSupport::LightOnly,
            _ => PaletteModeSupport::Both,
        }
    }
    /// Only rendering resolves a fixed mode. The stored preference is never rewritten.
    pub fn effective_mode(self, requested: ThemeMode, system: Option<egui::Theme>) -> ThemeMode {
        match self.mode_support() {
            PaletteModeSupport::LightOnly => ThemeMode::Light,
            PaletteModeSupport::DarkOnly => ThemeMode::Dark,
            PaletteModeSupport::Both => match requested {
                ThemeMode::System if system == Some(egui::Theme::Dark) => ThemeMode::Dark,
                ThemeMode::System => ThemeMode::Light,
                mode => mode,
            },
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StylePreset {
    #[default]
    Studio,
    Manuscript,
    Technical,
    Focus,
    Ledger,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Density {
    Compact,
    #[default]
    Standard,
    Spacious,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccentChoice {
    #[default]
    Palette,
    Pine,
    Plum,
    Copper,
    Indigo,
    IceCyan,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyFamily {
    #[default]
    Sans,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearancePreferences {
    #[serde(rename = "mode", alias = "theme")]
    pub theme: ThemeMode,
    pub palette: PaletteId,
    pub style: StylePreset,
    pub density: Density,
    pub accent: AccentChoice,
    pub body_family: BodyFamily,
    pub body_size: f32,
    pub source_size: f32,
    pub line_spacing: f32,
    pub reading_width: f32,
    pub ui_scale: f32,
    pub high_contrast: bool,
    pub reduce_motion: bool,
}
impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            palette: PaletteId::Mist,
            style: StylePreset::Studio,
            density: Density::Standard,
            accent: AccentChoice::Palette,
            body_family: BodyFamily::Sans,
            body_size: 17.0,
            source_size: 15.0,
            line_spacing: 1.65,
            reading_width: 760.0,
            ui_scale: 1.0,
            high_contrast: false,
            reduce_motion: false,
        }
    }
}
impl AppearancePreferences {
    pub fn normalize(&mut self) {
        self.body_size = bounded(self.body_size, 17.0, 12.0, 36.0);
        self.source_size = bounded(self.source_size, 15.0, 12.0, 28.0);
        self.line_spacing = bounded(self.line_spacing, 1.65, 1.0, 2.1);
        self.reading_width = bounded(self.reading_width, 760.0, 480.0, 1400.0);
        self.ui_scale = bounded(self.ui_scale, 1.0, 0.8, 2.0);
    }
}

pub(crate) fn bounded(value: f32, default: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}
