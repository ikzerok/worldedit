//! v0.30 十七套不透明语义色；装饰边线不充当必要控件边界。
use super::{AccentChoice, PaletteId};
use egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub workspace: Color32,
    pub chrome: Color32,
    pub panel: Color32,
    pub document: Color32,
    pub raised: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub disabled: Color32,
    pub control_border: Color32,
    pub subtle_border: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_hover: Color32,
    pub selection: Color32,
    pub selection_text: Color32,
    pub hover: Color32,
    pub focus: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub info: Color32,
    pub invalid_background: Color32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntaxPalette {
    pub keyword: Color32,
    pub string: Color32,
    pub reference: Color32,
    pub number: Color32,
    pub tag: Color32,
    pub comment: Color32,
    pub plain: Color32,
}
const fn hex(n: u32) -> Color32 {
    Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}

pub(super) fn palette(id: PaletteId, light: bool) -> (Colors, SyntaxPalette) {
    let data = match (id, light) {
        (PaletteId::Mist, true) => [
            0xEEF1EC, 0xE5EAE3, 0xF4F6F0, 0xFCFDF8, 0xFFFFFF, 0x25342B, 0x55695C, 0x6B8170,
            0xCDD6CA, 0x27654D, 0xFFFFFF, 0x1D563F, 0xD4E9DB, 0x233C2C, 0xE3E8DD, 0x27654D,
        ],
        (PaletteId::Mist, false) => [
            0x151E19, 0x1B2821, 0x22332A, 0x19251E, 0x2B3D32, 0xEDF4E9, 0xB2C3B5, 0x87A990,
            0x394E40, 0xA8D5AE, 0x183121, 0xBEE3C3, 0x344F3D, 0xEBF7EC, 0x2C3D30, 0xB5DFBB,
        ],
        (PaletteId::Plum, true) => [
            0xF5F0F5, 0xEDE2EB, 0xF9F3F8, 0xFFFBFE, 0xFFFFFF, 0x382B38, 0x6A5167, 0x84657F,
            0xD8CBD5, 0x803F73, 0xFFFFFF, 0x6C315F, 0xEBD7E8, 0x4F294B, 0xE8E0E6, 0x803F73,
        ],
        (PaletteId::Plum, false) => [
            0x201923, 0x2B2030, 0x332738, 0x261D2A, 0x3A2D3E, 0xF5EAF4, 0xCBB4C8, 0xB390AE,
            0x553F56, 0xE4ADDB, 0x3B2037, 0xF0C7E9, 0x563D57, 0xFAEDF7, 0x443346, 0xEABDDF,
        ],
        (PaletteId::Copper, true) => [
            0xF4EDDE, 0xE9DCC4, 0xF8F0E1, 0xFFFAED, 0xFFFFFF, 0x382D1F, 0x6B573E, 0x897148,
            0xDACAAB, 0x963F21, 0xFFFFFF, 0x803218, 0xF3DCC5, 0x573322, 0xE9E0D0, 0x963F21,
        ],
        (PaletteId::Copper, false) => [
            0x231C15, 0x2D241A, 0x372C21, 0x2A2118, 0x403325, 0xF6ECD9, 0xCFBBA0, 0xB09B77,
            0x55432D, 0xF2B98A, 0x3C2515, 0xFFD0AB, 0x5B3E27, 0xFFF0DC, 0x493928, 0xF4C496,
        ],
        (PaletteId::SeaSalt, true) => [
            0xEDF3F6, 0xDEEAF0, 0xF1F7FA, 0xFAFDFF, 0xFFFFFF, 0x203548, 0x4A6579, 0x627F95,
            0xC5D5E0, 0x285D95, 0xFFFFFF, 0x1D4B7C, 0xD3E4F4, 0x203E60, 0xE0EAF1, 0x245E9A,
        ],
        (PaletteId::SeaSalt, false) => [
            0x121F2B, 0x192A3B, 0x1F3549, 0x152637, 0x283F54, 0xE9F3FC, 0xABC4DB, 0x7EA9C8,
            0x354E65, 0xA1CAF1, 0x17324C, 0xBEDCFA, 0x2D4A68, 0xEBF6FF, 0x263E54, 0xB1D9FB,
        ],
        (PaletteId::Graphite, true) => [
            0xEEF0F2, 0xE2E7EA, 0xF4F6F7, 0xFDFEFE, 0xFFFFFF, 0x29343B, 0x53636D, 0x71828C,
            0xCCD4D9, 0x14636E, 0xFFFFFF, 0x0F505A, 0xD5E8EB, 0x204249, 0xE1E9ED, 0x0C6572,
        ],
        (PaletteId::Graphite, false) => [
            0x191E22, 0x212A30, 0x29363E, 0x1E282E, 0x33444D, 0xEEF4F6, 0xBACBD2, 0x8AACB9,
            0x40515B, 0x98D4DE, 0x173A41, 0xB6E7EC, 0x354F5B, 0xECFAFC, 0x2E414B, 0xABE5ED,
        ],
        (PaletteId::Terminal, _) => [
            0x08130D, 0x0D1C13, 0x12261A, 0x0A180F, 0x193120, 0xD6F5D4, 0x9FC39F, 0x719D78,
            0x2C5236, 0x8DEF6F, 0x12270B, 0xAAF993, 0x234630, 0xE0FADF, 0x183623, 0xB4F991,
        ],
        (PaletteId::Neon, _) => [
            0x140F24, 0x1B1333, 0x231B3C, 0x17112D, 0x2E2348, 0xF2EFFF, 0xC0B5DC, 0xAA96CC,
            0x51426D, 0xF2A2DF, 0x39132F, 0xFFC1EA, 0x3C2B55, 0xFAF0FF, 0x2E2445, 0x7BE1EC,
        ],
        (PaletteId::Vellum, _) => [
            0xEEEBD8, 0xE4E1CA, 0xF4F1DF, 0xFBF8E7, 0xFFFDF1, 0x2C3027, 0x5C604D, 0x757B60,
            0xCCCBB5, 0x4A5930, 0xFFFFFF, 0x384723, 0xDFE5CC, 0x2F3C29, 0xE6E8D7, 0x623C4C,
        ],
        (PaletteId::Sakura, true) => [
            0xFBEFF1, 0xF4DFE6, 0xFDF4F6, 0xFFFAFB, 0xFFFFFF, 0x482D39, 0x775363, 0x986A7C,
            0xE2C9D4, 0x983A63, 0xFFFFFF, 0x7F2D50, 0xF3DCE6, 0x5C2942, 0xF4E4EA, 0x9C365F,
        ],
        (PaletteId::Sakura, false) => [
            0x28151E, 0x331B28, 0x3D2330, 0x301922, 0x482B3A, 0xFFEAF2, 0xDCB6C6, 0xBA8FA2,
            0x643D50, 0xF8AAC8, 0x4B1C32, 0xFFCADD, 0x573145, 0xFFEAF3, 0x48293A, 0xFFBAD6,
        ],
        (PaletteId::Monochrome, true) => [
            0xEEEEEE, 0xE2E2E2, 0xF5F5F5, 0xFDFDFD, 0xFFFFFF, 0x202020, 0x5C5C5C, 0x777777,
            0xCCCCCC, 0x333333, 0xFFFFFF, 0x161616, 0xDEDEDE, 0x202020, 0xE7E7E7, 0x161616,
        ],
        (PaletteId::Monochrome, false) => [
            0x111111, 0x1B1B1B, 0x252525, 0x171717, 0x303030, 0xF1F1F1, 0xBCBCBC, 0x9C9C9C,
            0x484848, 0xE0E0E0, 0x191919, 0xFFFFFF, 0x3D3D3D, 0xF8F8F8, 0x313131, 0xF2F2F2,
        ],
    }
    .map(hex);
    let [workspace, chrome, panel, document, raised, text, secondary, control_border, subtle_border, accent, on_accent, accent_hover, selection, selection_text, hover, focus] =
        data;
    let semantic = if light {
        [0xB42332, 0x79520F, 0x256549, 0x315F93]
    } else {
        [0xFFABB2, 0xE8C16F, 0x9FD0AA, 0xA5C8F3]
    }
    .map(hex);
    let colors = Colors {
        workspace,
        chrome,
        panel,
        document,
        raised,
        text,
        secondary,
        disabled: secondary,
        control_border,
        subtle_border,
        accent,
        on_accent,
        accent_hover,
        selection,
        selection_text,
        hover,
        focus,
        danger: semantic[0],
        warning: semantic[1],
        success: semantic[2],
        info: semantic[3],
        invalid_background: document,
    };
    let syntax = match (id, light) {
        (PaletteId::Mist, true) => [0x70449A, 0x276449, 0x2B627A, 0x8A5424, 0x87516B],
        (PaletteId::Mist, false) => [0xD3B1F0, 0xB1D79D, 0x92CBDF, 0xE7BE8B, 0xE2B0CB],
        (PaletteId::Plum, true) => [0x754294, 0x34654A, 0x3D5C89, 0x8C5024, 0x884366],
        (PaletteId::Plum, false) => [0xD5B0F1, 0xA8D4B5, 0xA6C7F2, 0xEBC095, 0xECA7C9],
        (PaletteId::Copper, true) => [0x7E427E, 0x456032, 0x345F75, 0x8C4C22, 0x925055],
        (PaletteId::Copper, false) => [0xD9AFE1, 0xB6CF91, 0xA4CDDF, 0xEFBE89, 0xE7ABAD],
        (PaletteId::SeaSalt, true) => [0x614D93, 0x28634F, 0x225E86, 0x8B4E24, 0x805272],
        (PaletteId::SeaSalt, false) => [0xC4B7F0, 0xA3D5B8, 0xA2D5F3, 0xE6BE92, 0xD8B5DD],
        (PaletteId::Graphite, true) => [0x634A8D, 0x326149, 0x285D78, 0x875226, 0x7E506D],
        (PaletteId::Graphite, false) => [0xCEBAEC, 0xB0D3B8, 0xA8D3E8, 0xE4C399, 0xDDB8D1],
        (PaletteId::Terminal, _) => [0xB4E98A, 0x80D7A0, 0x94D7D6, 0xE6D488, 0xC6E9C0],
        (PaletteId::Neon, _) => [0xEBA6F1, 0xACE6BF, 0x8BDCEB, 0xEFC19C, 0xBCAFF8],
        (PaletteId::Vellum, _) => [0x623C4C, 0x3C5F41, 0x365A69, 0x74522C, 0x635276],
        (PaletteId::Sakura, true) => [0x854777, 0x41654C, 0x355F85, 0x8A4B33, 0x904066],
        (PaletteId::Sakura, false) => [0xE8B3EF, 0xACD6B8, 0xA8CCE9, 0xEAC19F, 0xF3ACC9],
        (PaletteId::Monochrome, true) => [0x333333, 0x505050, 0x3F3F3F, 0x595959, 0x474747],
        (PaletteId::Monochrome, false) => [0xE8E8E8, 0xC3C3C3, 0xD6D6D6, 0xBABABA, 0xCECECE],
    }
    .map(hex);
    let [keyword, string, reference, number, tag] = syntax;
    (
        colors,
        SyntaxPalette {
            keyword,
            string,
            reference,
            number,
            tag,
            comment: secondary,
            plain: text,
        },
    )
}

pub(super) fn apply_accent(colors: &mut Colors, choice: AccentChoice, light: bool) {
    let id = match choice {
        AccentChoice::Palette => return,
        AccentChoice::Pine => PaletteId::Mist,
        AccentChoice::Plum => PaletteId::Plum,
        AccentChoice::Copper => PaletteId::Copper,
        AccentChoice::Indigo => PaletteId::SeaSalt,
        AccentChoice::IceCyan => PaletteId::Graphite,
    };
    let (accent, _) = palette(id, light);
    colors.accent = accent.accent;
    colors.on_accent = accent.on_accent;
    colors.accent_hover = accent.accent_hover;
    colors.focus = accent.focus;
    // Selection surfaces belong to the base palette. This keeps all prose, semantic and
    // syntax foregrounds paired with their verified surfaces when only accent changes.
}
