//! 仅桌面端的可选 CPU 帧样本记录。
//!
//! 记录器不参与默认运行，也不提供产品 UI；输出只用于开发和发布候选测试。

use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAX_SAMPLES: usize = 65_536;
const CSV_HEADER: &str = "frame_index,cpu_ms,map_active,input_active,pixels_per_point,elapsed_ms\n";

#[derive(Clone, Copy)]
struct FrameMetadata {
    frame_index: u64,
    map_active: bool,
    input_active: bool,
    pixels_per_point: f32,
    elapsed_ms: f64,
}

#[derive(Clone, Copy)]
struct FrameSample {
    metadata: FrameMetadata,
    cpu_ms: f64,
}

pub(super) struct FrameProfiler {
    path: Option<PathBuf>,
    samples: Vec<FrameSample>,
    previous: Option<FrameMetadata>,
    next_frame_index: u64,
    started_at: Instant,
    stopped: bool,
    invalid_cpu_reported: bool,
}

impl FrameProfiler {
    pub(super) fn from_env() -> Option<Self> {
        let value = std::env::var_os("WORLDEDIT_PROFILE_CSV")?;
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty() {
            eprintln!("性能采样未启用：WORLDEDIT_PROFILE_CSV 为空");
            return None;
        }
        Some(Self::from_path(path))
    }

    #[cfg(test)]
    fn new(path: PathBuf) -> Self {
        Self::from_path(path)
    }

    fn from_path(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            samples: Vec::with_capacity(MAX_SAMPLES),
            previous: None,
            next_frame_index: 0,
            started_at: Instant::now(),
            stopped: false,
            invalid_cpu_reported: false,
        }
    }

    pub(super) fn is_active(&self) -> bool {
        !self.stopped
    }

    pub(super) fn begin_frame(&mut self, cpu_usage: Option<f32>) {
        if self.stopped {
            return;
        }
        let Some(metadata) = self.previous.take() else {
            return;
        };
        let Some(cpu_seconds) = cpu_usage else {
            return;
        };
        if !cpu_seconds.is_finite() || cpu_seconds < 0.0 {
            if !self.invalid_cpu_reported {
                eprintln!("性能采样忽略非有限或负的 CPU 帧耗时");
                self.invalid_cpu_reported = true;
            }
            return;
        }
        let cpu_ms = f64::from(cpu_seconds) * 1000.0;
        if !cpu_ms.is_finite() {
            if !self.invalid_cpu_reported {
                eprintln!("性能采样忽略超出范围的 CPU 帧耗时");
                self.invalid_cpu_reported = true;
            }
            return;
        }
        self.samples.push(FrameSample { metadata, cpu_ms });
        if self.samples.len() == MAX_SAMPLES {
            self.flush_and_stop();
        }
    }

    pub(super) fn finish_frame(
        &mut self,
        map_active: bool,
        input_active: bool,
        pixels_per_point: f32,
    ) {
        if self.stopped {
            return;
        }
        self.previous = Some(FrameMetadata {
            frame_index: self.next_frame_index,
            map_active,
            input_active,
            pixels_per_point,
            elapsed_ms: self.started_at.elapsed().as_secs_f64() * 1000.0,
        });
        self.next_frame_index = self.next_frame_index.saturating_add(1);
    }

    fn flush_and_stop(&mut self) {
        self.stopped = true;
        let Some(path) = self.path.take() else {
            self.samples.clear();
            return;
        };
        let result = write_csv(&path, &self.samples);
        self.samples.clear();
        if let Err(error) = result {
            if error.kind() == io::ErrorKind::AlreadyExists {
                eprintln!("性能采样 CSV 拒绝覆盖已有文件：{}", path.display());
            } else {
                eprintln!("性能采样 CSV 写入失败：{}：{error}", path.display());
            }
        }
    }
}

impl Drop for FrameProfiler {
    fn drop(&mut self) {
        if !self.stopped {
            self.flush_and_stop();
        }
    }
}

fn write_csv(path: &Path, samples: &[FrameSample]) -> io::Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(CSV_HEADER.as_bytes())?;
    for sample in samples {
        let metadata = sample.metadata;
        writeln!(
            writer,
            "{},{:.6},{},{},{:.6},{:.6}",
            metadata.frame_index,
            sample.cpu_ms,
            metadata.map_active,
            metadata.input_active,
            metadata.pixels_per_point,
            metadata.elapsed_ms,
        )?;
    }
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::FrameProfiler;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_path() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "worldedit-frame-profile-{}-{}.csv",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn writes_previous_frame_metadata_as_csv() {
        let path = temp_path();
        let mut profiler = FrameProfiler::new(path.clone());
        profiler.finish_frame(true, true, 1.5);
        profiler.begin_frame(Some(0.012));
        profiler.finish_frame(false, false, 2.0);
        drop(profiler);

        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv.starts_with(
            "frame_index,cpu_ms,map_active,input_active,pixels_per_point,elapsed_ms\n"
        ));
        assert!(csv.contains("0,12.000000,true,true,1.500000,"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_cpu_samples_and_does_not_overwrite_existing_csv() {
        let path = temp_path();
        let mut profiler = FrameProfiler::new(path.clone());
        profiler.finish_frame(true, false, 1.0);
        profiler.begin_frame(Some(f32::NAN));
        profiler.finish_frame(false, true, 1.0);
        profiler.begin_frame(Some(0.003));
        profiler.finish_frame(true, true, 1.0);
        drop(profiler);
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(!csv.lines().any(|line| line.starts_with("0,")));
        assert!(csv.contains("1,3.000000,false,true,1.000000,"));

        let mut profiler = FrameProfiler::new(path.clone());
        profiler.finish_frame(true, true, 1.0);
        profiler.begin_frame(Some(0.001));
        drop(profiler);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), csv);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_environment_variable_disables_sampling() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("WORLDEDIT_PROFILE_CSV");
        std::env::remove_var("WORLDEDIT_PROFILE_CSV");
        assert!(FrameProfiler::from_env().is_none());
        if let Some(value) = previous {
            std::env::set_var("WORLDEDIT_PROFILE_CSV", value);
        }
    }
}
