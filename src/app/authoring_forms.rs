//! 表单只持有 core 草稿与打开时的基线；写入仍由 Project 校验。
use std::path::PathBuf;
use worldline_core::authoring::EntityDraft;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::project::Project;
use worldline_core::{RelationDraft, RelationTypeDraft};

#[derive(Clone)]
pub(super) struct FormGuard {
    version: u64,
    baseline: String,
}
impl FormGuard {
    pub fn capture(project: &Project, version: u64) -> Self {
        Self {
            version,
            baseline: project.content_baseline(),
        }
    }
    pub fn is_current(&self, project: &Project, version: u64) -> bool {
        self.version == version && self.baseline == project.content_baseline()
    }
    pub fn check(&self, project: &Project, version: u64) -> Result<(), String> {
        if self.is_current(project, version) {
            Ok(())
        } else {
            Err("表单打开后工程已变化。输入已保留；请复制所需内容，关闭并重新打开后合并。".into())
        }
    }
}

pub(super) fn next_id(catalog: &Catalog, kind: &str) -> String {
    (1..)
        .map(|i| format!("{kind}_{i}"))
        .find(|id| {
            if kind == "relation_type" {
                !catalog.relation_types.contains_key(id)
            } else {
                catalog.object(&TargetRef::new(kind, id)).is_none()
            }
        })
        .unwrap()
}

#[derive(Clone)]
pub(super) struct EntityForm {
    pub original: Option<String>,
    pub path: PathBuf,
    pub draft: EntityDraft,
    pub guard: FormGuard,
}
impl EntityForm {
    pub fn open(
        project: &Project,
        catalog: &Catalog,
        version: u64,
        id: Option<&str>,
        path: PathBuf,
    ) -> Result<Self, String> {
        let draft = if let Some(id) = id {
            let item = catalog.entities.get(id).ok_or("实体已不存在")?;
            EntityDraft {
                id: item.id.clone(),
                entity_type: item.entity_type.clone(),
                display: item.display.clone(),
                description: item.description.clone(),
                properties: item.properties.clone().into_iter().collect(),
            }
        } else {
            EntityDraft {
                id: next_id(catalog, "entity"),
                entity_type: "place".into(),
                ..Default::default()
            }
        };
        let path = id
            .and_then(|id| catalog.entities.get(id))
            .map(|item| PathBuf::from(&item.file))
            .unwrap_or(path);
        Ok(Self {
            original: id.map(str::to_owned),
            path,
            draft,
            guard: FormGuard::capture(project, version),
        })
    }
    pub fn apply(&self, project: &mut Project, version: u64) -> Result<(), String> {
        self.guard.check(project, version)?;
        project.edit(|p| p.write_entity(&self.path, self.original.as_deref(), &self.draft))
    }
}

#[derive(Clone)]
pub(super) struct RelationForm {
    pub original: Option<String>,
    pub draft: RelationDraft,
    pub guard: FormGuard,
    pub from_search: String,
    pub to_search: String,
}
impl RelationForm {
    pub fn open(
        project: &Project,
        catalog: &Catalog,
        version: u64,
        id: Option<&str>,
        from: Option<TargetRef>,
    ) -> Result<Self, String> {
        let draft = if let Some(id) = id {
            let item = catalog.relations.get(id).ok_or("关系已不存在")?;
            RelationDraft {
                id: item.id.clone(),
                relation_type: item.relation_type.clone(),
                from: item.from_ref.clone(),
                to: item.to_ref.clone(),
                description: item.description.clone(),
                source_note: item.source_note.clone(),
                scope_refs: item.scope_refs.clone(),
                properties: item.properties.clone().into_iter().collect(),
            }
        } else {
            RelationDraft {
                id: next_id(catalog, "relation"),
                from: from.unwrap_or_default(),
                ..Default::default()
            }
        };
        Ok(Self {
            original: id.map(str::to_owned),
            draft,
            guard: FormGuard::capture(project, version),
            from_search: String::new(),
            to_search: String::new(),
        })
    }
    pub fn apply(&self, project: &mut Project, version: u64) -> Result<(), String> {
        self.guard.check(project, version)?;
        project.write_relation(self.original.as_deref(), &self.draft)
    }
}

#[derive(Clone)]
pub(super) struct RelationTypeForm {
    pub original: Option<String>,
    pub draft: RelationTypeDraft,
    pub guard: FormGuard,
}
impl RelationTypeForm {
    pub fn open(
        project: &Project,
        catalog: &Catalog,
        version: u64,
        id: Option<&str>,
    ) -> Result<Self, String> {
        let draft = if let Some(id) = id {
            let item = catalog.relation_types.get(id).ok_or("关系类型已不存在")?;
            RelationTypeDraft {
                id: item.id.clone(),
                display: item.display.clone(),
                inverse_display: item.inverse_display.clone(),
                direction: item.direction,
                from_kind: item.from_kind.clone(),
                to_kind: item.to_kind.clone(),
            }
        } else {
            RelationTypeDraft {
                id: next_id(catalog, "relation_type"),
                ..Default::default()
            }
        };
        Ok(Self {
            original: id.map(str::to_owned),
            draft,
            guard: FormGuard::capture(project, version),
        })
    }
    pub fn apply(&self, project: &mut Project, version: u64) -> Result<(), String> {
        self.guard.check(project, version)?;
        project.write_relation_type(self.original.as_deref(), &self.draft)
    }
}

pub(super) struct DeleteForm {
    pub impact: worldline_core::reference_impact::DeletionImpact,
    pub guard: FormGuard,
    pub confirmed: bool,
}
impl DeleteForm {
    pub fn open(project: &Project, version: u64, target: TargetRef) -> Self {
        Self {
            impact: project.deletion_impact(&target),
            guard: FormGuard::capture(project, version),
            confirmed: false,
        }
    }
    pub fn apply(&self, project: &mut Project, version: u64) -> Result<(), String> {
        self.guard.check(project, version)?;
        if !self.confirmed || !self.impact.can_delete() {
            return Err("请检查并解除引用，再明确确认删除；取消不会写入任何文件。".into());
        }
        let target = &self.impact.target;
        match target.kind.as_str() {
            "entity" => project.edit(|p| p.remove_entity(&target.id)),
            "relation" => project.remove_relation(&target.id),
            _ => Err("此面板仅处理通用实体和独立关系".into()),
        }
    }
}
