use super::*;

#[test]
fn registry_has_ten_distinct_families_and_seventeen_effective_members() {
    let mut names = std::collections::BTreeSet::new();
    let mut members = std::collections::BTreeSet::new();
    for palette in PaletteId::ALL {
        let name = serde_json::to_string(&palette).unwrap();
        assert!(names.insert(name.clone()), "duplicate palette registration");
        assert!(!palette.label().is_empty() && !palette.description().is_empty());
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            let t = resolve(
                &AppearancePreferences {
                    palette,
                    theme,
                    ..Default::default()
                },
                None,
            );
            members.insert((name.clone(), t.light));
            assert_eq!(t.preferences.theme, theme);
            assert_eq!(
                t.effective_mode,
                if t.light {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            );
            assert_eq!(serde_json::from_str::<PaletteId>(&name).unwrap(), palette);
        }
    }
    assert_eq!(names.len(), 10);
    assert_eq!(members.len(), 17);
}

#[test]
fn fixed_modes_preserve_requests_and_switching_back_restores_each_intent() {
    for (palette, effective) in [
        (PaletteId::Terminal, ThemeMode::Dark),
        (PaletteId::Neon, ThemeMode::Dark),
        (PaletteId::Vellum, ThemeMode::Light),
    ] {
        for requested in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            for system in [None, Some(egui::Theme::Light), Some(egui::Theme::Dark)] {
                let mut p = AppearancePreferences {
                    palette,
                    theme: requested,
                    ..Default::default()
                };
                p.normalize();
                let fixed = resolve(&p, system);
                assert_eq!(p.theme, requested, "normalization must not rewrite intent");
                assert_eq!(fixed.preferences, p);
                assert_eq!(fixed.effective_mode, effective);
                assert!(palette
                    .mode_support()
                    .notice()
                    .unwrap()
                    .contains("已保留原模式偏好"));
                p.palette = PaletteId::Mist;
                let restored = resolve(&p, system);
                assert_eq!(restored.preferences.theme, requested);
                assert_eq!(
                    restored.effective_mode,
                    match requested {
                        ThemeMode::System if system == Some(egui::Theme::Dark) => ThemeMode::Dark,
                        ThemeMode::System => ThemeMode::Light,
                        mode => mode,
                    }
                );
                assert_eq!(
                    fixed.type_roles, restored.type_roles,
                    "palette cannot change fonts"
                );
                assert_eq!(
                    fixed.metrics, restored.metrics,
                    "palette cannot change layout"
                );
            }
        }
    }
}

#[test]
fn fixed_system_theme_does_not_reinstall_on_os_change_and_context_remains_scoped() {
    for palette in [PaletteId::Terminal, PaletteId::Neon, PaletteId::Vellum] {
        let ctx = egui::Context::default();
        let p = AppearancePreferences {
            palette,
            ..Default::default()
        };
        let mut first = None;
        for system in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(system),
                    ..Default::default()
                },
                |ctx| {
                    let _guard = configure_appearance(ctx, &p);
                    let actual = resolved(ctx);
                    assert_eq!(actual.preferences.theme, ThemeMode::System);
                    assert_eq!(ctx.style().visuals.dark_mode, !actual.light);
                    assert_eq!(BG(), actual.colors.workspace);
                    assert_eq!(actual, resolve(&p, Some(system)));
                    if let Some(revision) = first {
                        assert_eq!(actual.revision, revision);
                        assert_eq!(ctx.style().spacing.tooltip_width, 389.0);
                    } else {
                        first = Some(actual.revision);
                        ctx.style_mut(|style| style.spacing.tooltip_width = 389.0);
                    }
                },
            );
        }
        assert_eq!(
            BG(),
            resolve(&AppearancePreferences::default(), None)
                .colors
                .workspace
        );
    }
}

#[test]
fn monochrome_is_neutral_except_explicit_semantics_and_optional_accent() {
    for theme in [ThemeMode::Light, ThemeMode::Dark] {
        let t = resolve(
            &AppearancePreferences {
                palette: PaletteId::Monochrome,
                theme,
                ..Default::default()
            },
            None,
        );
        let c = t.colors;
        for color in [
            c.workspace,
            c.chrome,
            c.panel,
            c.document,
            c.raised,
            c.text,
            c.secondary,
            c.control_border,
            c.subtle_border,
            c.accent,
            c.focus,
            c.hover,
            c.selection,
            t.syntax.keyword,
            t.syntax.string,
            t.syntax.reference,
            t.syntax.number,
            t.syntax.tag,
        ] {
            assert_eq!(color.r(), color.g());
            assert_eq!(color.g(), color.b());
        }
        assert_ne!(c.danger.r(), c.danger.g());
        assert_ne!(c.warning.r(), c.warning.b());
        assert!(PaletteId::Monochrome.description().contains("错误、警告"));
    }
}

#[test]
fn source_highlighting_uses_each_resolved_palette_without_changing_source_or_font() {
    let source = "event start\n  \"中文字符串\" #tag {value}\n  -> finish\n// 注释\n";
    for palette in PaletteId::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            let ctx = egui::Context::default();
            let p = AppearancePreferences {
                palette,
                theme,
                source_size: 19.0,
                ..Default::default()
            };
            let _guard = configure_appearance(&ctx, &p);
            let t = resolved(&ctx);
            let job = crate::highlight::layout_job(
                source,
                p.source_size,
                worldline_core::LanguageVersion::V1_12,
            );
            assert_eq!(job.text, source);
            for expected in [
                t.syntax.keyword,
                t.syntax.string,
                t.syntax.reference,
                t.syntax.tag,
                t.syntax.comment,
                t.syntax.plain,
            ] {
                assert!(
                    job.sections
                        .iter()
                        .any(|section| section.format.color == expected),
                    "{palette:?}/{theme:?} must use each source role"
                );
            }
            let mut end = 0;
            for section in &job.sections {
                assert_eq!(section.byte_range.start, end);
                assert!(source.is_char_boundary(section.byte_range.start));
                assert!(source.is_char_boundary(section.byte_range.end));
                assert_eq!(section.format.font_id, egui::FontId::monospace(19.0));
                end = section.byte_range.end;
            }
            assert_eq!(end, source.len());
        }
    }
}

#[test]
fn all_family_requested_modes_resolve_consistently_for_each_system_state() {
    let mut checked = 0;
    for palette in PaletteId::ALL {
        for requested in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            for system in [None, Some(egui::Theme::Light), Some(egui::Theme::Dark)] {
                let t = resolve(
                    &AppearancePreferences {
                        palette,
                        theme: requested,
                        ..Default::default()
                    },
                    system,
                );
                let expected = match palette.mode_support() {
                    PaletteModeSupport::DarkOnly => ThemeMode::Dark,
                    PaletteModeSupport::LightOnly => ThemeMode::Light,
                    PaletteModeSupport::Both => match requested {
                        ThemeMode::System if system == Some(egui::Theme::Dark) => ThemeMode::Dark,
                        ThemeMode::System => ThemeMode::Light,
                        explicit => explicit,
                    },
                };
                assert_eq!(t.preferences.theme, requested);
                assert_eq!(t.effective_mode, expected);
                assert_eq!(t.light, expected == ThemeMode::Light);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 90);
}
