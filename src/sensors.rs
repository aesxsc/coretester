use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct SensorSnapshot {
    pub temperature_c: Option<f64>,
    pub energy_uj: Option<u64>,
    pub max_energy_uj: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct SensorDelta {
    pub power_w: Option<f64>,
}

#[cfg(target_os = "linux")]
fn read_number(path: &std::path::Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn temperature() -> Option<f64> {
    use std::fs;
    use std::path::Path;
    let mut values = Vec::new();
    for root in [
        Path::new("/sys/class/thermal"),
        Path::new("/sys/class/hwmon"),
    ] {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if root.ends_with("thermal") {
                if let Some(v) = read_number(&path.join("temp")) {
                    if v < 200_000 {
                        values.push(v as f64 / 1000.0);
                    }
                }
            } else if let Ok(files) = fs::read_dir(&path) {
                for file in files.flatten() {
                    let name = file.file_name().to_string_lossy().to_string();
                    if name.starts_with("temp") && name.ends_with("_input") {
                        if let Some(v) = read_number(&file.path()) {
                            if v < 200_000 {
                                values.push(v as f64 / 1000.0);
                            }
                        }
                    }
                }
            }
        }
    }
    values
        .into_iter()
        .filter(|v| v.is_finite() && *v > 0.0)
        .max_by(|a, b| a.total_cmp(b))
}

#[cfg(target_os = "linux")]
fn energy() -> (Option<u64>, Option<u64>) {
    use std::fs;
    use std::path::{Path, PathBuf};
    fn visit(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        if depth > 4 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                visit(&p, out, depth + 1);
            } else if p.file_name().is_some_and(|n| n == "energy_uj") {
                out.push(p);
            }
        }
    }
    let mut paths = Vec::new();
    visit(Path::new("/sys/class/powercap"), &mut paths, 0);
    // Use only the shallowest package-level counter to avoid double counting subdomains.
    paths.sort_by_key(|p| p.components().count());
    let Some(path) = paths.first() else {
        return (None, None);
    };
    let value = read_number(path);
    let max = path
        .parent()
        .and_then(|p| read_number(&p.join("max_energy_range_uj")));
    (value, max)
}

pub fn snapshot() -> SensorSnapshot {
    #[cfg(target_os = "linux")]
    {
        let (energy_uj, max_energy_uj) = energy();
        SensorSnapshot {
            temperature_c: temperature(),
            energy_uj,
            max_energy_uj,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        SensorSnapshot::default()
    }
}

pub fn delta(before: &SensorSnapshot, after: &SensorSnapshot, elapsed: Duration) -> SensorDelta {
    let power_w = match (before.energy_uj, after.energy_uj) {
        (Some(a), Some(b)) if elapsed.as_secs_f64() > 0.0 => {
            let diff = if b >= a {
                b - a
            } else {
                before
                    .max_energy_uj
                    .map(|m| m.saturating_sub(a).saturating_add(b))
                    .unwrap_or(0)
            };
            if diff > 0 {
                Some(diff as f64 / 1_000_000.0 / elapsed.as_secs_f64())
            } else {
                None
            }
        }
        _ => None,
    };
    SensorDelta { power_w }
}

pub fn description() -> String {
    #[cfg(target_os = "linux")]
    {
        let s = snapshot();
        match (s.temperature_c, s.energy_uj) {
            (Some(_), Some(_)) => "Linux hwmon/thermal temperature and powercap package energy available; values are package-level samples".into(),
            (Some(_), None) => "Linux temperature available; powercap energy unavailable or permission denied; values are package-level samples".into(),
            (None, Some(_)) => "Linux powercap package energy available; temperature unavailable; values are package-level samples".into(),
            (None, None) => "OS hardware sensors unavailable or permission denied; no values were estimated".into(),
        }
    }
    #[cfg(target_os = "windows")]
    {
        "Stock Windows has no dependable unprivileged package temperature/energy API; sensor values are unavailable, not estimated".into()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "No sensor backend is implemented for this operating system".into()
    }
}
