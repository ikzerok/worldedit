//! 同键 schema 迁移，逐字段修复外观；未来数据永不被旧编辑器自动覆盖。
use super::{PersonalState, Settings, KEY};
use crate::theme::{AppearancePreferences, ThemeMode};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn field<T: DeserializeOwned>(value: &Value, name: &str, fallback: T) -> T {
    value
        .get(name)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(fallback)
}
// Unit enums also deserialize from externally tagged maps by default. Device
// choices use only the saved string format, matching the loading-shell whitelist.
fn choice<T: DeserializeOwned>(value: &Value, name: &str, fallback: T) -> T {
    if value.get(name).is_some_and(Value::is_string) {
        field(value, name, fallback)
    } else {
        fallback
    }
}
fn appearance(value: &Value, legacy: bool) -> AppearancePreferences {
    let defaults = if legacy {
        AppearancePreferences {
            body_size: 16.0,
            source_size: 16.0,
            line_spacing: 1.45,
            reading_width: 840.0,
            ..Default::default()
        }
    } else {
        AppearancePreferences::default()
    };
    let theme = if legacy {
        choice(value, "theme", ThemeMode::Dark)
    } else {
        choice(value, "mode", choice(value, "theme", defaults.theme))
    };
    let mut p = AppearancePreferences {
        theme,
        palette: choice(value, "palette", defaults.palette),
        style: choice(value, "style", defaults.style),
        density: choice(value, "density", defaults.density),
        accent: choice(value, "accent", defaults.accent),
        body_family: choice(value, "body_family", defaults.body_family),
        body_size: field(value, "body_size", defaults.body_size),
        source_size: field(
            value,
            "source_size",
            if legacy {
                field(value, "body_size", defaults.source_size)
            } else {
                defaults.source_size
            },
        ),
        line_spacing: field(value, "line_spacing", defaults.line_spacing),
        reading_width: field(value, "reading_width", defaults.reading_width),
        ui_scale: field(value, "ui_scale", defaults.ui_scale),
        high_contrast: field(value, "high_contrast", defaults.high_contrast),
        reduce_motion: field(value, "reduce_motion", defaults.reduce_motion),
    };
    p.normalize();
    p
}
fn settings(value: &Value, legacy: bool) -> Settings {
    let defaults = Settings::default();
    let mut s = Settings {
        appearance: appearance(if legacy { value } else { &value["appearance"] }, legacy),
        navigation: field(value, "navigation", defaults.navigation),
        diagnostics: field(value, "diagnostics", defaults.diagnostics),
        source_wrap: field(value, "source_wrap", defaults.source_wrap),
        focus: field(value, "focus", defaults.focus),
        dock_references: field(value, "dock_references", defaults.dock_references),
        references_visible: field(value, "references_visible", defaults.references_visible),
        navigation_width: field(value, "navigation_width", defaults.navigation_width),
        reference_width: field(value, "reference_width", defaults.reference_width),
    };
    s.normalize();
    s
}
impl PersonalState {
    pub(in crate::app) fn storage_warning(&self) -> Option<&str> {
        self.storage_notice.as_deref()
    }
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let text = storage.and_then(|s| s.get_string(KEY));
        #[cfg(target_arch = "wasm32")]
        let text = text.or_else(|| {
            web_sys::window()?
                .local_storage()
                .ok()??
                .get_item(KEY)
                .ok()?
        });
        let Some(text) = text else {
            return Self::default();
        };
        Self::decode(&text).unwrap_or_else(|| Self {
            protected_storage: Some(text),
            storage_notice: Some("设备设置无法完整读取，原始数据已保留；本次个人设置与作者位置不会写回，作品保存不受影响".into()),
            ..Default::default()
        })
    }
    pub(super) fn decode(text: &str) -> Option<Self> {
        if text.len() > 1_048_576 {
            return None;
        }
        let mut json: Value = serde_json::from_str(text).ok()?;
        let object = json.as_object_mut()?;
        let version = match object.get("schema_version") {
            Some(value) => value.as_u64()?,
            None => 1,
        };
        if version == 0 {
            return None;
        }
        let legacy = version == 1;
        let s = settings(object.get("settings").unwrap_or(&Value::Null), legacy);
        object.insert("settings".into(), serde_json::to_value(s).ok()?);
        object.insert("schema_version".into(), Value::from(2));
        let mut state: Self = serde_json::from_value(json).ok()?;
        if state.workspaces.len() > 64 {
            return None;
        }
        for workspace in state.workspaces.values_mut() {
            workspace.references.truncate(2);
        }
        if version > 2 {
            state.protected_storage = Some(text.to_owned());
            state.storage_notice = Some(format!(
                "设备设置由较新版本（schema {version}）保存；原始数据受保护，本次个人设置与作者位置不会写回，作品保存不受影响"
            ));
        }
        Some(state)
    }
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if self.protected_storage.is_some() {
            return;
        }
        if let Ok(text) = serde_json::to_string(self) {
            storage.set_string(KEY, text);
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn save_browser(&mut self) {
        if self.protected_storage.is_some() {
            return;
        }
        if let (Some(storage), Ok(text)) = (
            web_sys::window().and_then(|w| w.local_storage().ok().flatten()),
            serde_json::to_string(self),
        ) {
            if self.last_browser_saved != text {
                match storage.set_item(KEY, &text) {
                    Ok(()) => {
                        self.last_browser_saved = text;
                        self.storage_notice = None;
                    }
                    Err(_) => self.storage_notice = Some(
                        "浏览器未能保存个人设置与作者位置；当前显示仍保留，请检查存储权限或空间"
                            .into(),
                    ),
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn last_project(&self) -> Option<std::path::PathBuf> {
        self.last_project.clone()
    }
}
