//! 原先已审阅的十个成员逐 token 锁定，扩展不能悄悄改动它们。
use super::*;
#[test]
fn original_ten_members_keep_every_reviewed_base_and_syntax_hex() {
    for (palette, theme, expected) in [
        (PaletteId::Mist, ThemeMode::Light, "EEF1EC E5EAE3 F4F6F0 FCFDF8 FFFFFF 25342B 55695C 6B8170 CDD6CA 27654D FFFFFF 1D563F D4E9DB 233C2C E3E8DD 27654D 70449A 276449 2B627A 8A5424 87516B"),
        (PaletteId::Mist, ThemeMode::Dark, "151E19 1B2821 22332A 19251E 2B3D32 EDF4E9 B2C3B5 87A990 394E40 A8D5AE 183121 BEE3C3 344F3D EBF7EC 2C3D30 B5DFBB D3B1F0 B1D79D 92CBDF E7BE8B E2B0CB"),
        (PaletteId::Plum, ThemeMode::Light, "F5F0F5 EDE2EB F9F3F8 FFFBFE FFFFFF 382B38 6A5167 84657F D8CBD5 803F73 FFFFFF 6C315F EBD7E8 4F294B E8E0E6 803F73 754294 34654A 3D5C89 8C5024 884366"),
        (PaletteId::Plum, ThemeMode::Dark, "201923 2B2030 332738 261D2A 3A2D3E F5EAF4 CBB4C8 B390AE 553F56 E4ADDB 3B2037 F0C7E9 563D57 FAEDF7 443346 EABDDF D5B0F1 A8D4B5 A6C7F2 EBC095 ECA7C9"),
        (PaletteId::Copper, ThemeMode::Light, "F4EDDE E9DCC4 F8F0E1 FFFAED FFFFFF 382D1F 6B573E 897148 DACAAB 963F21 FFFFFF 803218 F3DCC5 573322 E9E0D0 963F21 7E427E 456032 345F75 8C4C22 925055"),
        (PaletteId::Copper, ThemeMode::Dark, "231C15 2D241A 372C21 2A2118 403325 F6ECD9 CFBBA0 B09B77 55432D F2B98A 3C2515 FFD0AB 5B3E27 FFF0DC 493928 F4C496 D9AFE1 B6CF91 A4CDDF EFBE89 E7ABAD"),
        (PaletteId::SeaSalt, ThemeMode::Light, "EDF3F6 DEEAF0 F1F7FA FAFDFF FFFFFF 203548 4A6579 627F95 C5D5E0 285D95 FFFFFF 1D4B7C D3E4F4 203E60 E0EAF1 245E9A 614D93 28634F 225E86 8B4E24 805272"),
        (PaletteId::SeaSalt, ThemeMode::Dark, "121F2B 192A3B 1F3549 152637 283F54 E9F3FC ABC4DB 7EA9C8 354E65 A1CAF1 17324C BEDCFA 2D4A68 EBF6FF 263E54 B1D9FB C4B7F0 A3D5B8 A2D5F3 E6BE92 D8B5DD"),
        (PaletteId::Graphite, ThemeMode::Light, "EEF0F2 E2E7EA F4F6F7 FDFEFE FFFFFF 29343B 53636D 71828C CCD4D9 14636E FFFFFF 0F505A D5E8EB 204249 E1E9ED 0C6572 634A8D 326149 285D78 875226 7E506D"),
        (PaletteId::Graphite, ThemeMode::Dark, "191E22 212A30 29363E 1E282E 33444D EEF4F6 BACBD2 8AACB9 40515B 98D4DE 173A41 B6E7EC 354F5B ECFAFC 2E414B ABE5ED CEBAEC B0D3B8 A8D3E8 E4C399 DDB8D1"),
    ] {
        let t = resolve(&AppearancePreferences { palette, theme, ..Default::default() }, None);
        let c = t.colors;
        let actual = [c.workspace, c.chrome, c.panel, c.document, c.raised, c.text, c.secondary,
            c.control_border, c.subtle_border, c.accent, c.on_accent, c.accent_hover,
            c.selection, c.selection_text, c.hover, c.focus, t.syntax.keyword, t.syntax.string,
            t.syntax.reference, t.syntax.number, t.syntax.tag];
        let expected: Vec<_> = expected.split_whitespace().collect();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_eq!(format!("{:02X}{:02X}{:02X}", actual.r(), actual.g(), actual.b()), expected,
                "{palette:?}/{theme:?}");
            assert_eq!(actual.a(), 255);
        }
        assert_eq!(c.disabled, c.secondary);
        assert_eq!(c.invalid_background, c.document);
        assert_eq!(t.syntax.comment, c.secondary);
        assert_eq!(t.syntax.plain, c.text);
    }
}
