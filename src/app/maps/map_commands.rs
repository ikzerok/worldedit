use super::*;
impl super::super::WorldeditApp {
    pub(super) fn map_command_baseline(&self, map_id: &str) -> Option<MapCommandBaseline> {
        let path =
            worldline_core::presentation_commands::map_document_path(&self.project, map_id).ok()?;
        let document = self.project.authoring_document(&path).ok()?;
        let mut expected_documents = std::collections::BTreeMap::new();
        expected_documents.insert(
            path,
            worldline_core::presentation_commands::document_hash(document.bytes()),
        );
        Some(MapCommandBaseline {
            revision: self.map_revision,
            expected_documents,
        })
    }

    pub(super) fn apply_map_command(
        &mut self,
        map_id: &str,
        command: worldline_core::presentation_commands::Command,
        label: &str,
    ) -> bool {
        self.apply_map_command_with_baseline(map_id, command, label, None)
    }

    pub(super) fn apply_map_command_with_baseline(
        &mut self,
        map_id: &str,
        command: worldline_core::presentation_commands::Command,
        label: &str,
        baseline: Option<MapCommandBaseline>,
    ) -> bool {
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式".into());
            return false;
        }
        let path =
            match worldline_core::presentation_commands::map_document_path(&self.project, map_id) {
                Ok(path) => path,
                Err(error) => {
                    self.io_error = Some(error.to_string());
                    return false;
                }
            };
        let baseline = baseline
            .or_else(|| self.map_canvas.command_baseline.clone())
            .or_else(|| {
                let expected = self
                    .project
                    .authoring_document(&path)
                    .ok()
                    .map(|document| {
                        worldline_core::presentation_commands::document_hash(document.bytes())
                    })?;
                let mut expected_documents = std::collections::BTreeMap::new();
                expected_documents.insert(path.clone(), expected);
                Some(MapCommandBaseline {
                    revision: self.map_revision,
                    expected_documents,
                })
            });
        let Some(baseline) = baseline else {
            self.io_error = Some("无法读取地图文档基线".into());
            return false;
        };
        let before = self.project.clone();
        let envelope = worldline_core::presentation_commands::CommandEnvelope {
            expected_revision: baseline.revision,
            expected_documents: baseline.expected_documents,
            command,
        };
        let content = self.snapshot.as_ref().map(|snapshot| &snapshot.result);
        let result = match content {
            Some(content) => worldline_core::presentation_commands::apply_with_content(
                &mut self.project,
                &mut self.map_revision,
                envelope,
                content,
            ),
            None => worldline_core::presentation_commands::apply(
                &mut self.project,
                &mut self.map_revision,
                envelope,
            ),
        };
        match result {
            Ok(result) => {
                self.remember(before);
                self.map_canvas.reset_local_preview();
                self.refresh_presentation_after_map_command();
                self.io_error = None;
                self.message = Some(label.into());
                self.map_failed_command = None;
                debug_assert_eq!(self.map_revision, result.new_revision);
                true
            }
            Err(error) => {
                self.io_error = Some(error.to_string());
                false
            }
        }
    }

    fn remember_failed_map_command(&mut self, map_id: &str, intent: EditIntent) {
        self.map_failed_command = Some(PendingMapCommand {
            map_id: map_id.to_owned(),
            intent,
        });
    }

    pub(super) fn retry_failed_map_command(&mut self) {
        let Some(pending) = self.map_failed_command.take() else {
            return;
        };
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式，再按当前版本重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        }
        let Some(current_map_id) = self.map_selection.as_deref() else {
            self.io_error = Some("当前没有选中的地图，无法重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        };
        if current_map_id != pending.map_id {
            self.io_error = Some(format!(
                "待重试命令属于地图“{}”，请返回该地图后按当前版本重试提交",
                pending.map_id
            ));
            self.map_failed_command = Some(pending);
            return;
        }
        let Some(baseline) = self.map_command_baseline(&pending.map_id) else {
            self.io_error = Some("无法读取当前地图文档基线，暂不能重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        };
        self.message = Some("按当前地图版本重新检查并提交展示预览".into());
        self.apply_map_intents(vec![pending.intent], vec![Some(baseline)]);
    }

    pub(super) fn apply_map_intents(
        &mut self,
        intents: Vec<EditIntent>,
        baselines: Vec<Option<MapCommandBaseline>>,
    ) {
        let Some(map_id) = self.map_selection.clone() else {
            return;
        };
        for (index, intent) in intents.into_iter().enumerate() {
            let baseline = baselines.get(index).cloned().unwrap_or(None);
            match intent {
                EditIntent::Move {
                    placement,
                    geometry,
                } => {
                    let pending = EditIntent::Move {
                        placement: placement.clone(),
                        geometry: geometry.clone(),
                    };
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::UpdatePlacement {
                            map_id: map_id.clone(),
                            placement_id: placement.clone(),
                            geometry: Some(core_geometry(&geometry)),
                            target_ref: None,
                            annotation: None,
                            role: None,
                            label_override: None,
                            layer_id: None,
                        },
                        "已保存标记位置（可撤销）",
                        baseline.clone(),
                    ) {
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    } else {
                        self.map_form.clear();
                    }
                }
                EditIntent::Create(geometry) => {
                    let pending = EditIntent::Create(geometry.clone());
                    if self.map_form.pending_place.is_some() {
                        self.io_error = Some("请先提交或取消已保留的地点落点".into());
                        self.map_canvas.restore_failed_preview(&pending);
                        break;
                    }
                    let Some(layer_id) = self
                        .map_canvas
                        .layer_details()
                        .into_iter()
                        .find(|(_, _, visible, locked, _)| *visible && !*locked)
                        .map(|(id, _, _, _, _)| id)
                    else {
                        self.io_error = Some("当前没有可编辑的未锁定图层".into());
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    };
                    if let MapGeometry::Text {
                        position,
                        text,
                        font_size,
                        color,
                    } = &geometry
                    {
                        if self.map_form.has_uncommitted_work() {
                            self.io_error = Some("请先保存或取消当前表单，再放置文字".into());
                            self.map_canvas.reset_local_preview();
                            break;
                        }
                        self.map_form.text_draft = Some(text_labels::TextDraft {
                            map_id: map_id.clone(),
                            layer_id,
                            placement_id: None,
                            position: *position,
                            text: text.clone(),
                            font_size: *font_size,
                            color: color.clone(),
                            baseline: baseline.clone(),
                        });
                        self.map_canvas.reset_local_preview();
                        self.map_canvas.set_tool(CanvasTool::Select);
                        self.message = Some("文字落点已保留；填写文字后保存".into());
                        break;
                    }
                    if self.map_form.create_place_on_next_point {
                        if !matches!(geometry, MapGeometry::Point(_)) {
                            self.io_error = Some("新建地点入口需要地图上的点落位".into());
                            self.map_canvas.restore_failed_preview(&pending);
                            break;
                        }
                        let source_path = if self.project.documents.contains_key(&self.active_file)
                        {
                            self.active_file.clone()
                        } else {
                            self.project.entry.clone()
                        };
                        let entity_id = self
                            .snapshot
                            .as_ref()
                            .map(|snapshot| {
                                crate::app::authoring_forms::next_id(
                                    &snapshot.result.analysis.catalog,
                                    "entity",
                                )
                            })
                            .unwrap_or_else(|| "entity_1".into());
                        self.map_form.pending_place = Some(PendingPlace {
                            map_id: map_id.clone(),
                            layer_id,
                            geometry,
                            expected_baseline: self.project.content_baseline(),
                            entity_id,
                            placement_id: self.next_map_placement_id(&map_id),
                            source_path,
                        });
                        self.map_form.create_place_on_next_point = false;
                        self.map_canvas.reset_local_preview();
                        self.message = Some("落点已保留；填写地点资料后再一起提交入口".into());
                        break;
                    }
                    let placement_id = self.next_map_placement_id(&map_id);
                    let annotation = if self.map_form.annotation.trim().is_empty() {
                        "地图标记".into()
                    } else {
                        self.map_form.annotation.trim().into()
                    };
                    let role = if self.map_form.role.trim().is_empty() {
                        "说明".into()
                    } else {
                        self.map_form.role.trim().into()
                    };
                    let label_override = (!self.map_form.label_override.trim().is_empty())
                        .then(|| self.map_form.label_override.trim().to_owned());
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::CreatePlacement {
                            map_id: map_id.clone(),
                            placement_id,
                            layer_id,
                            target_ref: self.map_form.target.clone(),
                            geometry: core_geometry(&geometry),
                            annotation,
                            role,
                            label_override,
                        },
                        "已保存地图标记",
                        baseline.clone(),
                    ) {
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    } else {
                        self.map_form.clear();
                    }
                }
                EditIntent::Delete { placement } => {
                    let pending = EditIntent::Delete {
                        placement: placement.clone(),
                    };
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::DeletePlacement {
                            map_id: map_id.clone(),
                            placement_id: placement,
                        },
                        "已删除地图标记（可撤销）",
                        baseline.clone(),
                    ) {
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    }
                    self.map_form.clear();
                }
            }
        }
    }

    pub(super) fn next_map_placement_id(&self, map_id: &str) -> String {
        let mut index = 1;
        let existing = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map_index.maps.get(map_id));
        while existing.is_some_and(|map| map.placements.contains_key(&format!("marker_{index}"))) {
            index += 1;
        }
        format!("marker_{index}")
    }
}
