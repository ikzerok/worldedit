//! UTF-8 安全预览和可键盘展开的完整值；空值、未初始化与类型不混淆。
use super::{super::focus::FocusReveal, Value};

pub(super) fn display(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "未初始化（尚无此全局变量）".into();
    };
    let text = value.display();
    let text = if matches!(value, Value::Str(_)) && text.is_empty() {
        "\"\"（空字符串）".into()
    } else {
        text
    };
    format!("{text} · {}", value.kind_label())
}

pub(super) fn preview(text: &str) -> (String, bool) {
    const LIMIT: usize = 160;
    let count = text.chars().count();
    if count <= LIMIT {
        return (text.to_owned(), false);
    }
    (
        format!(
            "{}…（已截断，共 {count} 字符）",
            text.chars().take(LIMIT).collect::<String>()
        ),
        true,
    )
}

pub(super) fn render(ui: &mut egui::Ui, label: &str, value: Option<&Value>, focus: FocusReveal) {
    ui.push_id(label, |ui| {
        let full = display(value);
        let (short, truncated) = preview(&full);
        ui.add(egui::Label::new(format!("{label}：{short}")).wrap());
        if truncated {
            let output = egui::CollapsingHeader::new(format!("查看{label}完整值"))
                .id_salt("full-value")
                .show(ui, |ui| {
                    ui.add(egui::Label::new(&full).wrap());
                    let response = ui.button("复制完整值");
                    focus.reveal(ui, &response);
                    if response.clicked() {
                        ui.ctx()
                            .copy_text(value.map(Value::display).unwrap_or_default());
                    }
                });
            focus.reveal(ui, &output.header_response);
        }
    });
}
