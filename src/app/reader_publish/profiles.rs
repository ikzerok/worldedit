use super::super::WorldeditApp;
use super::*;
use worldline_core::project::Project;
use worldline_core::reader_export::{READER_SITE_SCHEMA_VERSION, READER_STORY_FEATURE};

impl ReaderPublishState {
    pub(in crate::app) fn refresh_profiles(&mut self, project: &Project) {
        match project.reader_profiles() {
            Ok(profiles) => {
                self.profiles = profiles;
                self.profile_error = None;
            }
            Err(error) => {
                self.profiles.clear();
                self.profile_error = Some(error);
            }
        }
    }

    pub(super) fn load_profile(&mut self, profile: ReaderPublicationProfile) {
        let selection = &profile.selection;
        self.site_title = selection.site_title.clone();
        self.objects = selection.objects.iter().cloned().collect();
        self.fields = selection
            .fields
            .iter()
            .map(|field| (field.target.clone(), field.keys.iter().cloned().collect()))
            .collect();
        self.maps = selection
            .maps
            .iter()
            .map(|map| (map.id.clone(), map.clone()))
            .collect();
        self.chapters = selection
            .manuscripts
            .iter()
            .map(|book| (book.id.clone(), book.chapters.iter().cloned().collect()))
            .collect();
        self.attachments = selection.attachments.iter().cloned().collect();
        self.story_details = selection
            .required_features
            .iter()
            .any(|feature| feature == READER_STORY_FEATURE);
        self.profile_id = profile.id.clone();
        self.profile_title = profile.title.clone();
        self.profile = Some(profile);
        self.migration = None;
        self.migration_origin = None;
        self.invalidate_review();
        self.step = PublishStep::Select;
    }

    pub(super) fn profile_controls(&mut self, ui: &mut egui::Ui) -> Option<PublishAction> {
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("已保存发布配置", |ui| {
                for (index, profile) in self.profiles.iter().enumerate() {
                    if ui
                        .button(format!("{} · {}", profile.title, profile.id))
                        .clicked()
                    {
                        action = Some(PublishAction::LoadProfile(index));
                        ui.close();
                    }
                }
                if self.profiles.is_empty() {
                    ui.label("尚无可加载配置");
                }
            });
            if ui.button("新建选择").clicked() {
                action = Some(PublishAction::NewProfile);
            }
            ui.label(
                self.profile
                    .as_ref()
                    .map_or("临时选择", |_| "已载配置；修改后请应用配置"),
            );
        });
        if let Some(error) = &self.profile_error {
            ui.colored_label(crate::theme::ERROR(), error);
        }
        egui::CollapsingHeader::new("保存此发布配置")
            .id_salt("reader-profile-save")
            .show(ui, |ui| {
                ui.label("配置 ID（字母、数字、_ 或 -）");
                ui.add_enabled(
                    self.profile.is_none(),
                    egui::TextEdit::singleline(&mut self.profile_id),
                );
                ui.label("配置名称");
                if ui.text_edit_singleline(&mut self.profile_title).changed() {
                    self.invalidate_review();
                }
                if ui.button("应用配置到工程").clicked() {
                    action = Some(PublishAction::SaveProfile);
                }
                ui.label("应用可撤销；仍须“保存全部”才写入工作区。");
            });
        if self
            .profile
            .as_ref()
            .is_some_and(|profile| profile.selection.schema_version != READER_SITE_SCHEMA_VERSION)
            && ui.button("预览升级到世界网站格式").clicked()
        {
            action = Some(PublishAction::PreviewMigration);
        }
        if let Some(plan) = &self.migration {
            ui.group(|ui| {
                ui.strong("升级将增加公开语义，请明确确认");
                for change in &plan.authorization_changes {
                    ui.label(change);
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("确认升级候选（不保存）").clicked() {
                        action = Some(PublishAction::ConfirmMigration);
                    }
                    if ui.button("取消升级").clicked() {
                        action = Some(PublishAction::CancelMigration);
                    }
                });
            });
        }
        action
    }
}

impl WorldeditApp {
    pub(super) fn reader_profile_action(&mut self, action: PublishAction, ctx: &egui::Context) {
        match action {
            PublishAction::LoadProfile(index) => {
                if let Some(profile) = self.reader_publish.profiles.get(index).cloned() {
                    self.reader_publish.load_profile(profile);
                }
            }
            PublishAction::NewProfile => {
                let mut state = ReaderPublishState::new();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    state.destination = self.reader_publish.destination.clone();
                }
                state.refresh_choices(&self.project, self.snapshot.as_ref());
                state.open = true;
                self.reader_publish = state;
            }
            PublishAction::SaveProfile => self.start_reader_profile_save(ctx),
            PublishAction::PreviewMigration => {
                if let Some(profile) = self.reader_publish.current_profile() {
                    match self.project.preview_reader_profile_migration(&profile) {
                        Ok(plan) => {
                            self.reader_publish.migration_origin = Some(profile);
                            self.reader_publish.migration = Some(plan);
                        }
                        Err(error) => self.reader_publish.status = Some(error),
                    }
                }
            }
            PublishAction::ConfirmMigration => {
                if let Some(plan) = self.reader_publish.migration.take() {
                    if self.reader_publish.current_profile()
                        != self.reader_publish.migration_origin.take()
                    {
                        self.reader_publish.status = Some("配置已改变，请重新预览升级。".into());
                    } else {
                        match self.project.apply_reader_profile_migration(&plan) {
                            Ok(profile) => self.reader_publish.load_profile(profile),
                            Err(error) => self.reader_publish.status = Some(error),
                        }
                    }
                }
            }
            PublishAction::CancelMigration => {
                self.reader_publish.migration = None;
                self.reader_publish.migration_origin = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
