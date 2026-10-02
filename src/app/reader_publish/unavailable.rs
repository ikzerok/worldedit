use super::*;

#[derive(Clone)]
enum Missing {
    Object(TargetRef),
    Field(TargetRef, String),
    Map(String),
    Placement(String, String),
    Raster(String, String),
    Chapter(String, String),
    Attachment(String),
}

impl Missing {
    fn label(&self) -> String {
        match self {
            Self::Object(target) => format!("资料 {}:{}", target.kind, target.id),
            Self::Field(target, key) => format!("属性 {}:{} / {key}", target.kind, target.id),
            Self::Map(id) => format!("地图 {id}"),
            Self::Placement(map, id) => format!("地图图元 {map} / {id}"),
            Self::Raster(map, id) => format!("地图底图 {map} / {id}"),
            Self::Chapter(book, id) => format!("章节 {book} / {id}"),
            Self::Attachment(id) => format!("附件 {id}"),
        }
    }
}

impl ReaderPublishState {
    fn unavailable_items(&self) -> Vec<Missing> {
        let mut items = Vec::new();
        let objects: BTreeMap<_, _> = self
            .object_choices
            .iter()
            .map(|choice| (&choice.target, choice))
            .collect();
        let maps: BTreeMap<_, _> = self
            .map_choices
            .iter()
            .map(|choice| (choice.id.as_str(), choice))
            .collect();
        let books: BTreeMap<_, _> = self
            .manuscript_choices
            .iter()
            .filter(|book| book.unavailable.is_none())
            .map(|book| {
                (
                    book.id.as_str(),
                    book.chapters
                        .iter()
                        .map(|(id, _)| id.as_str())
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect();
        let attachments: BTreeSet<_> = self
            .attachment_choices
            .iter()
            .filter(|choice| choice.available)
            .map(|choice| choice.id.as_str())
            .collect();
        for target in &self.objects {
            if !objects.contains_key(target) {
                items.push(Missing::Object(target.clone()));
            }
        }
        for (target, keys) in &self.fields {
            for key in keys {
                if !self.objects.contains(target)
                    || !objects
                        .get(target)
                        .is_some_and(|choice| choice.fields.iter().any(|field| field.key == *key))
                {
                    items.push(Missing::Field(target.clone(), key.clone()));
                }
            }
        }
        for (id, selection) in &self.maps {
            if let Some(choice) = maps.get(id.as_str()) {
                let placements: BTreeSet<_> = choice
                    .placements
                    .iter()
                    .map(|(id, _)| id.as_str())
                    .collect();
                let rasters: BTreeSet<_> =
                    choice.rasters.iter().map(|(id, _)| id.as_str()).collect();
                for node in &selection.placements {
                    if !placements.contains(node.as_str()) {
                        items.push(Missing::Placement(id.clone(), node.clone()));
                    }
                }
                for raster in &selection.raster_layers {
                    if !rasters.contains(raster.as_str()) {
                        items.push(Missing::Raster(id.clone(), raster.clone()));
                    }
                }
            } else {
                items.push(Missing::Map(id.clone()));
            }
        }
        for (book, chapters) in &self.chapters {
            for chapter in chapters {
                if !books
                    .get(book.as_str())
                    .is_some_and(|chapters| chapters.contains(chapter.as_str()))
                {
                    items.push(Missing::Chapter(book.clone(), chapter.clone()));
                }
            }
        }
        for id in &self.attachments {
            if !attachments.contains(id.as_str()) {
                items.push(Missing::Attachment(id.clone()));
            }
        }
        items
    }

    fn remove_unavailable(&mut self, item: &Missing) {
        match item {
            Missing::Object(target) => {
                self.objects.remove(target);
                self.fields.remove(target);
            }
            Missing::Field(target, key) => {
                if let Some(keys) = self.fields.get_mut(target) {
                    keys.remove(key);
                }
            }
            Missing::Map(id) => {
                self.maps.remove(id);
            }
            Missing::Placement(map, id) => {
                if let Some(map) = self.maps.get_mut(map) {
                    map.placements.retain(|value| value != id);
                }
            }
            Missing::Raster(map, id) => {
                if let Some(map) = self.maps.get_mut(map) {
                    map.raster_layers.retain(|value| value != id);
                }
            }
            Missing::Chapter(book, id) => {
                if let Some(chapters) = self.chapters.get_mut(book) {
                    chapters.remove(id);
                }
            }
            Missing::Attachment(id) => {
                self.attachments.remove(id);
            }
        }
    }

    pub(super) fn unavailable_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let items = self.unavailable_items();
        if items.is_empty() {
            return false;
        }
        let mut changed = false;
        ui.separator();
        ui.colored_label(
            crate::theme::WARNING(),
            format!("{} 项已选内容当前不可用，未自动取消授权", items.len()),
        );
        ui.label("保留这些条目会让核心预览明确失败；仅在确认不再公开时逐项移除。");
        let range = selection_ui::page_range(ui, &mut self.unavailable_page, items.len());
        for (index, item) in items[range].iter().enumerate() {
            ui.push_id(("missing-reader-choice", index), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(item.label());
                    if ui.button("明确移除此项").clicked() {
                        self.remove_unavailable(item);
                        changed = true;
                    }
                });
            });
        }
        changed
    }
}
