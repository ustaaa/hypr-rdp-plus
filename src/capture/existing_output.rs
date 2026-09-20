//! Transactional, session-scoped resizing of an explicitly selected output.
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Mode {
    pub name: String,
    pub width: u32,
    pub height: u32,
    refresh: f64,
    x: i64,
    y: i64,
    scale: f64,
    transform: u64,
}
impl Mode {
    fn from_monitors(monitors: &Value, name: &str) -> Result<Self> {
        let m = monitors
            .as_array()
            .context("invalid monitor list")?
            .iter()
            .find(|m| m["name"].as_str() == Some(name))
            .context("selected output is absent")?;
        let mode = Self {
            name: name.to_owned(),
            width: u32::try_from(m["width"].as_u64().context("missing width")?)?,
            height: u32::try_from(m["height"].as_u64().context("missing height")?)?,
            refresh: m["refreshRate"].as_f64().context("missing refresh")?,
            x: m["x"].as_i64().context("missing x")?,
            y: m["y"].as_i64().context("missing y")?,
            scale: m["scale"].as_f64().context("missing scale")?,
            transform: m["transform"].as_u64().context("missing transform")?,
        };
        anyhow::ensure!(
            mode.width > 0
                && mode.height > 0
                && mode.scale.is_finite()
                && mode.scale > 0.0
                && mode.refresh.is_finite()
                && mode.refresh > 0.0
                && mode.transform <= 7,
            "invalid monitor geometry"
        );
        Ok(mode)
    }
    fn requested(&self, width: u32, height: u32) -> Result<Self> {
        anyhow::ensure!(
            (200..=3840).contains(&width)
                && (200..=3840).contains(&height)
                && u64::from(width) * u64::from(height) <= 3840 * 2400
                && width % 2 == 0
                && height % 2 == 0,
            "unsupported client dimensions"
        );
        // Quarter-turn transforms swap native and capture axes. Until the
        // capture protocol's coordinate conversion is negotiated, keep those
        // outputs unchanged rather than apply an ambiguous physical mode.
        anyhow::ensure!(
            self.transform % 2 == 0,
            "resizing quarter-turn outputs is not supported"
        );
        let mut requested = self.clone();
        requested.width = width;
        requested.height = height;
        Ok(requested)
    }
    fn matches(&self, other: &Self) -> bool {
        self.name == other.name
            && self.width == other.width
            && self.height == other.height
            && self.x == other.x
            && self.y == other.y
            && self.transform == other.transform
            && (self.scale - other.scale).abs() < 0.001
            && (self.refresh - other.refresh).abs() < 0.1
    }
}

pub(super) trait MonitorIo: Send {
    fn read(&mut self, name: &str) -> Result<Mode>;
    fn apply(&mut self, mode: &Mode) -> Result<()>;
}
struct HyprlandIo;
impl MonitorIo for HyprlandIo {
    fn read(&mut self, name: &str) -> Result<Mode> {
        Mode::from_monitors(&crate::hyprland::monitors()?, name)
    }
    fn apply(&mut self, mode: &Mode) -> Result<()> {
        crate::hyprland::set_monitor_geometry(
            &mode.name,
            mode.width,
            mode.height,
            mode.refresh,
            mode.x,
            mode.y,
            mode.scale,
            mode.transform,
        )?;
        let start = Instant::now();
        loop {
            if self
                .read(&mode.name)
                .is_ok_and(|actual| mode.matches(&actual))
            {
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(2) {
                bail!("compositor did not accept requested monitor mode");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub(super) struct ExistingOutput {
    name: String,
    original: Option<Mode>,
    io: Box<dyn MonitorIo>,
}
impl ExistingOutput {
    pub fn new(name: String) -> Self {
        Self {
            name,
            original: None,
            io: Box::new(HyprlandIo),
        }
    }
    pub fn resize<T>(
        &mut self,
        width: u32,
        height: u32,
        prepare: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let previous = self.io.read(&self.name)?;
        let requested = previous.requested(width, height)?;
        if self.original.is_none() {
            self.original = Some(previous.clone());
        }
        let result = self.io.apply(&requested).and_then(|()| prepare());
        if result.is_err() {
            if let Err(error) = self.io.apply(&previous) {
                tracing::error!(%error, "Existing output resize rollback failed; original retained for cleanup");
            }
        }
        result
    }
    pub fn restore(&mut self) -> Result<Option<(u32, u32)>> {
        let Some(original) = self.original.as_ref() else {
            return Ok(None);
        };
        self.io.apply(original)?;
        let dimensions = (original.width, original.height);
        self.original = None;
        Ok(Some(dimensions))
    }
}
impl Drop for ExistingOutput {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            tracing::error!(%error, "Could not restore original output on cleanup");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    fn mode() -> Mode {
        Mode::from_monitors(
            &serde_json::json!([{"name":"Virtual-1", "width":2560,"height":1080,
            "refreshRate":60.0,"x":-100,"y":20,"scale":1.25,"transform":2}]),
            "Virtual-1",
        )
        .unwrap()
    }
    struct Fake {
        current: Arc<Mutex<Mode>>,
        fail: bool,
    }
    impl MonitorIo for Fake {
        fn read(&mut self, _: &str) -> Result<Mode> {
            Ok(self.current.lock().unwrap().clone())
        }
        fn apply(&mut self, mode: &Mode) -> Result<()> {
            *self.current.lock().unwrap() = mode.clone();
            if self.fail {
                self.fail = false;
                bail!("partial apply failed");
            }
            Ok(())
        }
    }
    fn controller(fail: bool) -> (ExistingOutput, Arc<Mutex<Mode>>) {
        let current = Arc::new(Mutex::new(mode()));
        (
            ExistingOutput {
                name: "Virtual-1".into(),
                original: None,
                io: Box::new(Fake {
                    current: Arc::clone(&current),
                    fail,
                }),
            },
            current,
        )
    }
    #[test]
    fn resize_preserves_geometry_then_restores_session_original() {
        let (mut c, current) = controller(false);
        c.resize(1920, 1080, || Ok(())).unwrap();
        assert_eq!(
            *current.lock().unwrap(),
            mode().requested(1920, 1080).unwrap()
        );
        c.resize(1280, 720, || Ok(())).unwrap();
        c.restore().unwrap();
        assert_eq!(*current.lock().unwrap(), mode());
        assert_eq!(c.restore().unwrap(), None);
    }
    #[test]
    fn failed_apply_and_failed_layout_prepare_roll_back() {
        let (mut c, current) = controller(true);
        assert!(c.resize(1920, 1080, || Ok(())).is_err());
        assert_eq!(*current.lock().unwrap(), mode());
        assert!(c
            .resize(1280, 720, || -> Result<()> { bail!("layout invalid") })
            .is_err());
        assert_eq!(*current.lock().unwrap(), mode());
    }
    #[test]
    fn drop_restores_and_invalid_size_never_mutates() {
        let (mut c, current) = controller(false);
        assert!(c.resize(0, 1080, || Ok(())).is_err());
        assert_eq!(*current.lock().unwrap(), mode());
        c.resize(1920, 1080, || Ok(())).unwrap();
        drop(c);
        assert_eq!(*current.lock().unwrap(), mode());
    }
    #[test]
    fn failed_restore_retains_original_for_retry() {
        let (mut c, current) = controller(false);
        c.resize(1920, 1080, || Ok(())).unwrap();
        c.io = Box::new(Fake {
            current: Arc::clone(&current),
            fail: true,
        });
        assert!(c.restore().is_err());
        assert!(c.original.is_some());
        assert_eq!(c.restore().unwrap(), Some((2560, 1080)));
        assert!(c.original.is_none());
    }

    #[test]
    fn quarter_turn_requests_are_rejected_without_changing_orientation() {
        let mut rotated = mode();
        rotated.transform = 1;
        assert!(rotated.requested(1920, 1080).is_err());
        assert_eq!(rotated.transform, 1);
    }
}
