//! 校准草稿与临时尺子；语义、尺寸权重及校验全部交给 core。
use super::*;
use worldline_core::presentation::{measurement_distance, validate_measurement, MapMeasurement};

#[derive(Clone, Debug, Default)]
pub(super) struct MeasurementState {
    pub(super) ruler: bool,
    pub(super) ruler_points: Vec<[f64; 2]>,
    pub(super) calibration: Option<CalibrationDraft>,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct CalibrationDraft {
    pub(super) points: Vec<[f64; 2]>,
    pub(super) distance: String,
    pub(super) unit: String,
    pub(super) baseline: Option<MapCommandBaseline>,
}

impl MapCanvas {
    pub(super) fn begin_calibration(&mut self) -> bool {
        if !self.is_edit_mode()
            || self.map_id().is_empty()
            || self.has_uncommitted_work()
            || self.measurement_blocked
        {
            return false;
        }
        self.measurement.ruler = false;
        self.measurement.error = None;
        self.measurement.calibration = Some(CalibrationDraft {
            points: Vec::new(),
            distance: self
                .snapshot
                .measurement
                .as_ref()
                .map(|m| m.distance.to_string())
                .unwrap_or_default(),
            unit: self
                .snapshot
                .measurement
                .as_ref()
                .map(|m| m.unit.clone())
                .unwrap_or_default(),
            baseline: self.command_baseline.clone(),
        });
        true
    }

    pub(super) fn set_ruler(&mut self, active: bool) {
        self.measurement.ruler =
            active && self.snapshot.measurement.is_some() && self.measurement.calibration.is_none();
        self.measurement.ruler_points.clear();
        self.measurement.error = None;
    }

    pub(super) fn calibration_active(&self) -> bool {
        self.is_edit_mode() && self.measurement.calibration.is_some()
    }

    pub(super) fn measurement_active(&self) -> bool {
        self.calibration_active() || self.measurement.ruler
    }

    pub(super) fn cancel_measurement(&mut self) -> bool {
        if self.measurement.calibration.is_none() && !self.measurement.ruler {
            return false;
        }
        self.measurement = Default::default();
        let mut kept = Vec::new();
        let mut baselines = Vec::new();
        for (index, intent) in self.edit_intents.drain(..).enumerate() {
            if !matches!(intent, EditIntent::SetMeasurement(_)) {
                kept.push(intent);
                baselines.push(self.intent_baselines.get(index).cloned().flatten());
            }
        }
        self.edit_intents = kept;
        self.intent_baselines = baselines;
        true
    }

    pub(super) fn measurement_click(&mut self, point: [f64; 2]) {
        self.measurement.error = None;
        if self.calibration_active() {
            if let Some(draft) = self.measurement.calibration.as_mut() {
                // 两点齐备后冻结参照线，避免填写表单时误点改变比例。
                if draft.points.len() < 2 {
                    draft.points.push(point);
                }
            }
        } else if self.measurement.ruler {
            if self.measurement.ruler_points.len() == 2 {
                self.measurement.ruler_points.clear();
            }
            self.measurement.ruler_points.push(point);
        }
    }

    pub(super) fn calibration_value(&self) -> Result<MapMeasurement, String> {
        let draft = self
            .measurement
            .calibration
            .as_ref()
            .ok_or("请先开始两点校准")?;
        let points: [[f64; 2]; 2] = draft
            .points
            .clone()
            .try_into()
            .map_err(|_| "请在地图上选择两个参照点")?;
        let value = MapMeasurement {
            points,
            distance: draft
                .distance
                .trim()
                .parse::<f64>()
                .map_err(|_| "请输入已知距离（支持科学计数法）")?,
            unit: draft.unit.clone(),
            extra: self
                .snapshot
                .measurement
                .as_ref()
                .map(|m| m.extra.clone())
                .unwrap_or_default(),
        };
        validate_measurement(&self.snapshot.canvas, &value)?;
        Ok(value)
    }

    pub(super) fn confirm_calibration(&mut self) -> bool {
        if !self.calibration_active() || !self.edit_intents.is_empty() {
            return false;
        }
        match self.calibration_value() {
            Ok(value) if self.snapshot.measurement.as_ref() == Some(&value) => {
                self.measurement.error = Some("校准未改变，无需保存".into());
                false
            }
            Ok(value) => {
                self.measurement.error = None;
                self.intent_baselines.push(
                    self.measurement
                        .calibration
                        .as_ref()
                        .and_then(|draft| draft.baseline.clone()),
                );
                self.edit_intents.push(EditIntent::SetMeasurement(value));
                true
            }
            Err(error) => {
                self.measurement.error = Some(error);
                false
            }
        }
    }

    pub(super) fn restore_calibration(&mut self, value: &MapMeasurement) {
        // 失败后保留原输入与基线，通用“按当前版本重试”才替换基线。
        if self.measurement.calibration.is_none() {
            self.measurement.calibration = Some(CalibrationDraft {
                points: value.points.to_vec(),
                distance: value.distance.to_string(),
                unit: value.unit.clone(),
                baseline: self.command_baseline.clone(),
            });
        }
    }

    pub(super) fn ruler_distance(&self) -> Result<f64, String> {
        let calibration = self.snapshot.measurement.as_ref().ok_or("此地图尚未校准")?;
        let points: [[f64; 2]; 2] = self
            .measurement
            .ruler_points
            .clone()
            .try_into()
            .map_err(|_| "请在地图上点选两个测量点")?;
        measurement_distance(&self.snapshot.canvas, calibration, points)
    }

    pub(super) fn measurement_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if response.dragged_by(egui::PointerButton::Primary) {
            self.camera.pan_by(ui.input(|input| input.pointer.delta()));
        }
        if response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let point = self.camera.screen_to_normalized(pointer, response.rect);
                self.measurement_click([f64::from(point.x), f64::from(point.y)]);
            }
        }
    }
}

/// 有界展示：极小正值永不因固定小数位被显示成 0。
pub(super) fn distance_label(distance: f64, unit: &str) -> String {
    let number = if !(0.001..1_000_000.0).contains(&distance) {
        format!("{distance:.4e}")
    } else {
        let number = format!("{distance:.4}");
        number
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    };
    format!("{number} {unit}")
}
