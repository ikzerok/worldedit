//! This workbench opts into the same local preview geometry as typed plans.
//! Its query and delivery validity remain in the existing core-backed host guards.
use super::*;
pub(super) use crate::app::writing_workspace::preview_navigation::{self as shared, Reader};
pub(super) fn control(ui: &egui::Ui, response: egui::Response) -> egui::Response {
    shared::reveal(ui, &response, true, true);
    response
}
pub(super) fn text(ui: &egui::Ui, response: &egui::Response) {
    shared::reveal(ui, response, true, false);
}
pub(super) fn add(ui: &mut egui::Ui, enabled: bool, widget: impl egui::Widget) -> egui::Response {
    let response = theme::add_enabled(ui, enabled, widget);
    control(ui, response)
}
pub(super) fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    add(ui, true, egui::Button::new(label))
}
pub(super) fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    add(ui, true, egui::Checkbox::new(value, label))
}
pub(super) fn selectable<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let response = ui.selectable_value(current, value, label);
    control(ui, response)
}
pub(super) fn header<R>(
    ui: &mut egui::Ui,
    header: egui::CollapsingHeader,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    let response = header.show(ui, body);
    control(ui, response.header_response.clone());
    response
}
pub(super) fn collapsing<R>(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    header(ui, egui::CollapsingHeader::new(label), body)
}

pub(super) fn input(ui: &mut egui::Ui, widget: impl egui::Widget) -> egui::Response {
    let response = ui.add(widget);
    text(ui, &response);
    response
}
