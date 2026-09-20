//! 地图子地图导航的个人状态。
//!
//! 该模块只保存地图浏览路径、镜头快照和 core 提供的导航 DTO。它不读取或写入
//! Project，也不建立地图之间的语义关系。

use worldline_core::presentation::{MapNavigation, MapPlacement};

pub const MAX_HISTORY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraState {
    pub zoom: f32,
    pub pan: [f32; 2],
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapNavigationDto {
    pub map_id: String,
    pub available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementNavigationDto {
    pub placement_id: String,
    pub navigation: Option<MapNavigationDto>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Breadcrumb {
    pub map_id: String,
    pub title: String,
}

impl Breadcrumb {
    pub fn new(map_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            map_id: map_id.into(),
            title: title.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapViewState {
    pub map_id: String,
    pub title: String,
    pub camera: CameraState,
}

pub struct MapNavigationController {
    current: MapViewState,
    history: Vec<MapViewState>,
}

impl MapNavigationController {
    pub fn new(map_id: impl Into<String>, title: impl Into<String>, camera: CameraState) -> Self {
        Self {
            current: MapViewState {
                map_id: map_id.into(),
                title: title.into(),
                camera,
            },
            history: Vec::new(),
        }
    }

    pub fn current(&self) -> &MapViewState {
        &self.current
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.history
            .iter()
            .chain(std::iter::once(&self.current))
            .map(|view| Breadcrumb::new(&view.map_id, &view.title))
            .collect()
    }

    /// 保存当前地图离开前的个人镜头，供下一次进入时的返回恢复使用。
    pub fn update_current_camera(&mut self, camera: CameraState) {
        self.current.camera = camera;
    }

    pub fn enter(
        &mut self,
        target: &MapNavigationDto,
        title: impl Into<String>,
        camera: CameraState,
    ) -> bool {
        if !target.available {
            return false;
        }

        if self.history.len() == MAX_HISTORY {
            self.history.remove(0);
        }
        self.history.push(self.current.clone());
        self.current = MapViewState {
            map_id: target.map_id.clone(),
            title: title.into(),
            camera,
        };
        true
    }

    pub fn back(&mut self) -> Option<MapViewState> {
        let previous = self.history.pop()?;
        self.current = previous;
        Some(self.current.clone())
    }
}

pub fn project_navigation(navigation: Option<&MapNavigation>) -> Option<MapNavigationDto> {
    navigation.map(|navigation| MapNavigationDto {
        map_id: navigation.map_id.clone(),
        available: navigation.available,
    })
}

pub fn project_placement_navigation(placement: &MapPlacement) -> PlacementNavigationDto {
    PlacementNavigationDto {
        placement_id: placement.id.clone(),
        navigation: project_navigation(placement.navigation.as_ref()),
    }
}
