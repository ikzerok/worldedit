//! Only genuine UI setup. Its pointer/wheel operations are not keyboard assertions.
use super::*;
impl Keyboard {
    fn wheel_outer(&mut self, out: &egui::FullOutput, delta: f32) {
        let (_, id, receiver) = self
            .areas
            .iter()
            .find(|(name, _, _)| name == "production-script-workbench")
            .cloned()
            .expect("actual outer area receiver drawn in this pass");
        let clip = out
            .shapes
            .iter()
            .map(|s| s.clip_rect)
            .filter(|clip| clip.contains_rect(receiver.interact_rect))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("paint clip contains actual outer receiver");
        let outer = receiver
            .rect
            .intersect(receiver.interact_rect)
            .intersect(clip)
            .shrink(1.0);
        let nested: Vec<_> = self
            .areas
            .iter()
            .filter(|(name, _, _)| name == "production-artifact-bytes")
            .map(|(_, _, r)| r.rect.intersect(r.interact_rect))
            .collect();
        let point = [
            egui::vec2(0.05, 0.05),
            egui::vec2(0.95, 0.05),
            egui::vec2(0.05, 0.95),
            egui::vec2(0.95, 0.95),
        ]
        .into_iter()
        .map(|fraction| outer.min + outer.size() * fraction)
        .find(|point| !nested.iter().any(|rect| rect.contains(*point)))
        .expect("wheel point belongs to outer receiver and excludes inner area");
        assert!(
            receiver.rect.contains(point)
                && receiver.interact_rect.contains(point)
                && clip.contains(point)
        );
        assert_eq!(self.flow.ctx.layer_id_at(point), Some(receiver.layer_id));
        let before = self.offsets();
        self.trace.push(format!("SETUP wheel id={id:?} receiver={:?}/{:?} clip={clip:?} point={point:?} delta={delta} before={before:?}",receiver.rect,receiver.interact_rect));
        self.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: Modifiers::NONE,
            },
        ]);
        self.frame(vec![]);
        self.trace
            .push(format!("SETUP wheel after={:?}", self.offsets()));
    }
    pub fn pointer(&mut self, label: &str) -> egui::FullOutput {
        let mut out = self.settle();
        for _ in 0..240 {
            if let Some((rect, clip)) = visible(&out, label) {
                assert!(clip.contains_rect(rect));
                self.trace.push(format!(
                    "SETUP pointer label={label:?} rect={rect:?} clip={clip:?} offsets={:?}",
                    self.offsets()
                ));
                for pressed in [true, false] {
                    out = self.frame(vec![
                        Event::PointerMoved(rect.center()),
                        Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ]);
                }
                return out;
            }
            // Each independent seek begins at the actual top, using wheel events.
            let offset = self
                .offsets()
                .into_iter()
                .find(|(n, _, _)| n == "production-script-workbench")
                .expect("actual outer scroll state")
                .2;
            if offset.y <= 0.5 {
                break;
            }
            self.wheel_outer(&out, 600.0);
            out = self.frame(vec![]);
        }
        for _ in 0..240 {
            if let Some((rect, clip)) = visible(&out, label) {
                self.trace.push(format!(
                    "SETUP pointer label={label:?} rect={rect:?} clip={clip:?} offsets={:?}",
                    self.offsets()
                ));
                for pressed in [true, false] {
                    out = self.frame(vec![
                        Event::PointerMoved(rect.center()),
                        Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ]);
                }
                return out;
            }
            self.wheel_outer(&out, -180.0);
            out = self.frame(vec![]);
        }
        panic!(
            "real pointer setup could not reveal {label}; {}",
            self.diagnostic(&out)
        );
    }
    pub fn setup_generated(&mut self) {
        self.pointer(ROLE_HINT);
        assert!(self.response.as_ref().is_some_and(
            |r| r.has_focus() && egui::TextEdit::load_state(&self.flow.ctx, r.id).is_some()
        ));
        self.type_value("a");
        assert_eq!(
            self.flow.app.manuscript.production.speaker,
            Some(TargetRef::new("character", "a"))
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.pointer("生成当前稿台本");
            self.finish_job();
            if self.flow.app.production_is_current() {
                break;
            }
            assert!(
                self.flow.notice().contains("前一个有界作业") && Instant::now() < deadline,
                "{}",
                self.flow.notice()
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!self.flow.app.manuscript.production.confirmed);
        self.trace.push(format!(
            "SETUP generated key={} baseline={} disk_files={}",
            self.flow.snapshot().key(),
            self.flow.app.project.content_baseline(),
            self.flow.disk().len()
        ));
    }
    pub fn setup_preview(&mut self, format: &str, confirm: bool) {
        if !self.flow.app.manuscript.production.export_open {
            self.pointer("准备私密交付");
        }
        self.pointer(format);
        self.pointer("预览完整交付字节");
        self.settle();
        assert!(
            self.flow.app.manuscript.production.artifact.is_some(),
            "{}",
            self.flow.notice()
        );
        assert!(!self.flow.app.manuscript.production.confirmed);
        let expected = self
            .flow
            .snapshot()
            .export(
                self.flow
                    .app
                    .manuscript
                    .production
                    .artifact_options
                    .as_ref()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            self.flow.artifact(),
            expected.bytes(),
            "UI preview must be the same core snapshot bytes"
        );
        if confirm {
            self.pointer(CONFIRM);
            assert!(self.flow.app.manuscript.production.confirmed);
        }
        self.trace.push(format!(
            "SETUP preview key={} bytes={} options={:?} confirmed={confirm}",
            self.flow.snapshot().key(),
            self.flow.artifact().len(),
            self.flow.app.manuscript.production.artifact_options
        ));
    }
}
