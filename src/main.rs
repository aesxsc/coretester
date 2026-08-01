mod affinity;
mod bench;
mod report;
mod sensors;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
struct Config {
    duration: Duration,
    accel_duration: Duration,
    cores: Option<Vec<usize>>,
    output: PathBuf,
    json: PathBuf,
    open: bool,
    quick: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuIdentity {
    pub name: String,
    pub architecture: String,
    pub logical_cpus: usize,
    pub selected_cpus: usize,
    pub features: Vec<String>,
    pub benchmarked_features: Vec<String>,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreResult {
    pub cpu: usize,
    pub package: Option<i32>,
    pub physical_core: Option<i32>,
    pub siblings: Vec<usize>,
    pub core_kind: String,
    pub integer_gops: f64,
    pub float_gflops: f64,
    pub memory_gbps: f64,
    pub latency_ns: f64,
    pub accel: BTreeMap<String, f64>,
    pub accel_units: BTreeMap<String, String>,
    pub temperature_before_c: Option<f64>,
    pub temperature_after_c: Option<f64>,
    pub package_power_w: Option<f64>,
    pub elapsed_ms: u64,
    pub score: f64,
    pub stability: f64,
    pub affinity_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub best_cpu: usize,
    pub worst_cpu: usize,
    pub fastest_latency_cpu: usize,
    pub fastest_memory_cpu: usize,
    pub score_spread_percent: f64,
    pub mean_score: f64,
    pub score_cv_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub schema_version: u32,
    pub timestamp_unix: u64,
    pub identity: CpuIdentity,
    pub config: RunConfig,
    pub sensor_note: String,
    pub cores: Vec<CoreResult>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    pub duration_ms: u64,
    pub acceleration_duration_ms: u64,
    pub isolated_sequential_run: bool,
}

fn usage() {
    println!(
        r#"CoreTester — per-logical-CPU benchmark

Usage: coretester [options]

  --quick                 Short smoke run (35 ms main / 15 ms acceleration)
  --duration-ms N         Duration of each main kernel (default 180)
  --cores LIST            CPU list/ranges, e.g. 0,2-5,8
  --output PATH           HTML output (default coretester-report.html)
  --json PATH             JSON output (default coretester-results.json)
  --no-open               Do not open the report automatically
  -h, --help              Show this help
"#
    );
}

fn parse_core_list(value: &str, total: usize) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for part in value.split(',').filter(|v| !v.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let start: usize = a.parse().map_err(|_| format!("invalid CPU: {a}"))?;
            let end: usize = b.parse().map_err(|_| format!("invalid CPU: {b}"))?;
            if start > end {
                return Err(format!("reversed CPU range: {part}"));
            }
            out.extend(start..=end);
        } else {
            out.push(part.parse().map_err(|_| format!("invalid CPU: {part}"))?);
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err("CPU list is empty".into());
    }
    if let Some(&bad) = out.iter().find(|&&c| c >= total) {
        return Err(format!(
            "CPU {bad} is outside 0..{}",
            total.saturating_sub(1)
        ));
    }
    Ok(out)
}

fn parse_args(total: usize) -> Result<Option<Config>, String> {
    let mut cfg = Config {
        duration: Duration::from_millis(180),
        accel_duration: Duration::from_millis(60),
        cores: None,
        output: PathBuf::from("coretester-report.html"),
        json: PathBuf::from("coretester-results.json"),
        open: true,
        quick: false,
    };
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                usage();
                return Ok(None);
            }
            "--quick" => {
                cfg.quick = true;
                cfg.duration = Duration::from_millis(35);
                cfg.accel_duration = Duration::from_millis(15);
            }
            "--no-open" => cfg.open = false,
            "--duration-ms" | "--cores" | "--output" | "--json" => {
                let flag = args[i].clone();
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag.as_str() {
                    "--duration-ms" => {
                        let ms: u64 = value
                            .parse()
                            .map_err(|_| "duration must be an integer".to_string())?;
                        if !(10..=10_000).contains(&ms) {
                            return Err("duration must be 10..10000 ms".into());
                        }
                        cfg.duration = Duration::from_millis(ms);
                        cfg.accel_duration = Duration::from_millis((ms / 3).max(10));
                    }
                    "--cores" => cfg.cores = Some(parse_core_list(value, total)?),
                    "--output" => cfg.output = PathBuf::from(value),
                    "--json" => cfg.json = PathBuf::from(value),
                    _ => unreachable!(),
                }
            }
            unknown => return Err(format!("unknown option: {unknown}")),
        }
        i += 1;
    }
    Ok(Some(cfg))
}

fn cpu_name() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let max = std::arch::x86_64::__cpuid(0x8000_0000).eax;
        if max >= 0x8000_0004 {
            let mut bytes = Vec::with_capacity(48);
            for leaf in 0x8000_0002..=0x8000_0004 {
                let r = std::arch::x86_64::__cpuid(leaf);
                bytes.extend_from_slice(&r.eax.to_le_bytes());
                bytes.extend_from_slice(&r.ebx.to_le_bytes());
                bytes.extend_from_slice(&r.ecx.to_le_bytes());
                bytes.extend_from_slice(&r.edx.to_le_bytes());
            }
            let name = String::from_utf8_lossy(&bytes)
                .trim_matches(char::from(0))
                .trim()
                .to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    #[cfg(target_arch = "x86")]
    {
        let max = std::arch::x86::__cpuid(0x8000_0000).eax;
        if max >= 0x8000_0004 {
            let mut bytes = Vec::with_capacity(48);
            for leaf in 0x8000_0002..=0x8000_0004 {
                let r = std::arch::x86::__cpuid(leaf);
                bytes.extend_from_slice(&r.eax.to_le_bytes());
                bytes.extend_from_slice(&r.ebx.to_le_bytes());
                bytes.extend_from_slice(&r.ecx.to_le_bytes());
                bytes.extend_from_slice(&r.edx.to_le_bytes());
            }
            let name = String::from_utf8_lossy(&bytes)
                .trim_matches(char::from(0))
                .trim()
                .to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(info) = fs::read_to_string("/proc/cpuinfo") {
            for line in info.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    if matches!(k.trim(), "model name" | "Hardware") {
                        return v.trim().to_string();
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(v) = env::var("PROCESSOR_IDENTIFIER") {
            return v;
        }
    }
    env::consts::ARCH.to_string()
}

fn normalize_scores(results: &mut [CoreResult]) {
    fn median(mut values: Vec<f64>) -> f64 {
        values.sort_by(|a, b| a.total_cmp(b));
        let n = values.len();
        if n.is_multiple_of(2) {
            (values[n / 2 - 1] + values[n / 2]) / 2.0
        } else {
            values[n / 2]
        }
    }
    let mi = median(results.iter().map(|r| r.integer_gops).collect());
    let mf = median(results.iter().map(|r| r.float_gflops).collect());
    let mm = median(results.iter().map(|r| r.memory_gbps).collect());
    let ml = median(results.iter().map(|r| r.latency_ns).collect());
    for r in results {
        let perf = 0.30 * r.integer_gops / mi.max(1e-9)
            + 0.30 * r.float_gflops / mf.max(1e-9)
            + 0.20 * r.memory_gbps / mm.max(1e-9)
            + 0.20 * ml / r.latency_ns.max(1e-9);
        r.score = perf * 100.0;
    }
}

fn summarize(results: &[CoreResult]) -> Summary {
    let best = results
        .iter()
        .max_by(|a, b| a.score.total_cmp(&b.score))
        .unwrap();
    let worst = results
        .iter()
        .min_by(|a, b| a.score.total_cmp(&b.score))
        .unwrap();
    let latency = results
        .iter()
        .min_by(|a, b| a.latency_ns.total_cmp(&b.latency_ns))
        .unwrap();
    let memory = results
        .iter()
        .max_by(|a, b| a.memory_gbps.total_cmp(&b.memory_gbps))
        .unwrap();
    let mean = results.iter().map(|r| r.score).sum::<f64>() / results.len() as f64;
    let variance = results
        .iter()
        .map(|r| (r.score - mean).powi(2))
        .sum::<f64>()
        / results.len() as f64;
    Summary {
        best_cpu: best.cpu,
        worst_cpu: worst.cpu,
        fastest_latency_cpu: latency.cpu,
        fastest_memory_cpu: memory.cpu,
        score_spread_percent: (best.score - worst.score) / worst.score.max(1e-9) * 100.0,
        mean_score: mean,
        score_cv_percent: variance.sqrt() / mean.max(1e-9) * 100.0,
    }
}

fn terminal_bar(value: f64, max: f64, width: usize) -> String {
    let n = ((value / max.max(1e-9)) * width as f64)
        .round()
        .clamp(0.0, width as f64) as usize;
    format!("{}{}", "█".repeat(n), "░".repeat(width - n))
}

fn main() {
    let total = affinity::logical_cpu_count();
    let cfg = match parse_args(total) {
        Ok(Some(c)) => c,
        Ok(None) => return,
        Err(e) => {
            eprintln!("error: {e}\n");
            usage();
            std::process::exit(2);
        }
    };
    let selected = cfg.cores.clone().unwrap_or_else(|| (0..total).collect());
    let features = bench::detected_features();
    let benchmarked = bench::benchmarked_features();
    let identity = CpuIdentity {
        name: cpu_name(),
        architecture: env::consts::ARCH.into(),
        logical_cpus: total,
        selected_cpus: selected.len(),
        features,
        benchmarked_features: benchmarked.clone(),
        os: format!("{} {}", env::consts::OS, env::consts::ARCH),
    };

    println!("\x1b[1;36mCORETESTER\x1b[0m  {}", identity.name);
    println!(
        "{} logical CPUs · {} selected · {} accelerated paths",
        total,
        selected.len(),
        benchmarked.len()
    );
    println!("Sequential isolated run; each workload is pinned to its target CPU.\n");

    let topology = affinity::topology(total);
    let mut results = Vec::new();
    let run_started = Instant::now();
    for (position, &cpu) in selected.iter().enumerate() {
        print!(
            "\r\x1b[2K[{:>2}/{:>2}] CPU {:>3}  pinning...",
            position + 1,
            selected.len(),
            cpu
        );
        io::stdout().flush().ok();
        let core_started = Instant::now();
        let guard = affinity::AffinityGuard::pin(cpu);
        let affinity_ok = guard.as_ref().map(|g| g.is_active()).unwrap_or(false);
        let before = sensors::snapshot();
        let mut sample = bench::run_main(cfg.duration);
        let (accel, units) = bench::run_acceleration(cfg.accel_duration);
        let after = sensors::snapshot();
        sample.stability =
            bench::stability_probe((cfg.duration / 3).max(Duration::from_millis(10)));
        drop(guard);
        let topo = topology.get(cpu).cloned().unwrap_or_default();
        let elapsed = core_started.elapsed();
        let sensor_delta = sensors::delta(&before, &after, elapsed);
        results.push(CoreResult {
            cpu,
            package: topo.package,
            physical_core: topo.core,
            siblings: topo.siblings,
            core_kind: bench::core_kind(cpu),
            integer_gops: sample.integer_gops,
            float_gflops: sample.float_gflops,
            memory_gbps: sample.memory_gbps,
            latency_ns: sample.latency_ns,
            accel,
            accel_units: units,
            temperature_before_c: before.temperature_c,
            temperature_after_c: after.temperature_c,
            package_power_w: sensor_delta.power_w,
            elapsed_ms: elapsed.as_millis() as u64,
            score: 0.0,
            stability: sample.stability,
            affinity_ok,
        });
        let r = results.last().unwrap();
        print!("\r\x1b[2K[{:>2}/{:>2}] CPU {:>3}  int {:>6.2} GOPS  fp {:>6.2} GFLOPS  mem {:>6.2} GB/s  lat {:>6.1} ns",
            position + 1, selected.len(), cpu, r.integer_gops, r.float_gflops, r.memory_gbps, r.latency_ns);
        println!();
    }
    normalize_scores(&mut results);
    let summary = summarize(&results);
    let run = RunResult {
        schema_version: 1,
        timestamp_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        identity,
        config: RunConfig {
            duration_ms: cfg.duration.as_millis() as u64,
            acceleration_duration_ms: cfg.accel_duration.as_millis() as u64,
            isolated_sequential_run: true,
        },
        sensor_note: sensors::description(),
        cores: results,
        summary,
    };

    println!("\n\x1b[1mPer-core composite (median = 100)\x1b[0m");
    let max_score = run.cores.iter().map(|r| r.score).fold(0.0, f64::max);
    for r in &run.cores {
        let tag = if r.cpu == run.summary.best_cpu {
            " \x1b[32mBEST\x1b[0m"
        } else if r.cpu == run.summary.worst_cpu {
            " \x1b[31mWORST\x1b[0m"
        } else {
            ""
        };
        println!(
            "CPU {:>3} {} {:>6.1}{}",
            r.cpu,
            terminal_bar(r.score, max_score, 30),
            r.score,
            tag
        );
    }
    println!(
        "\nBest CPU {} · Worst CPU {} · Spread {:.1}% · Consistency CV {:.2}% · elapsed {:.1}s",
        run.summary.best_cpu,
        run.summary.worst_cpu,
        run.summary.score_spread_percent,
        run.summary.score_cv_percent,
        run_started.elapsed().as_secs_f64()
    );

    if let Some(parent) = cfg.json.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).ok();
    }
    if let Some(parent) = cfg.output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).ok();
    }
    let json = serde_json::to_string_pretty(&run).expect("serialize results");
    fs::write(&cfg.json, &json).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", cfg.json.display());
        std::process::exit(1);
    });
    let html = report::render(&run);
    fs::write(&cfg.output, html).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", cfg.output.display());
        std::process::exit(1);
    });
    let html_path = fs::canonicalize(&cfg.output).unwrap_or(cfg.output.clone());
    let json_path = fs::canonicalize(&cfg.json).unwrap_or(cfg.json.clone());
    println!("HTML  {}", html_path.display());
    println!("JSON  {}", json_path.display());
    println!("Sensors: {}", run.sensor_note);
    if cfg.open {
        if let Err(e) = report::open(&html_path) {
            eprintln!("Could not open report automatically: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_deduplicates_cpu_ranges() {
        assert_eq!(parse_core_list("4,1-3,2", 8).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn rejects_out_of_range_cpu() {
        assert!(parse_core_list("0,8", 8).unwrap_err().contains("outside"));
    }

    #[test]
    fn score_normalization_centers_similar_cores() {
        let make = |cpu| CoreResult {
            cpu,
            package: None,
            physical_core: None,
            siblings: vec![cpu],
            core_kind: "Uniform/unknown".into(),
            integer_gops: 4.0,
            float_gflops: 8.0,
            memory_gbps: 20.0,
            latency_ns: 50.0,
            accel: BTreeMap::new(),
            accel_units: BTreeMap::new(),
            temperature_before_c: None,
            temperature_after_c: None,
            package_power_w: None,
            elapsed_ms: 1,
            score: 0.0,
            stability: 100.0,
            affinity_ok: true,
        };
        let mut cores = vec![make(0), make(1)];
        normalize_scores(&mut cores);
        assert!(cores.iter().all(|c| (c.score - 100.0).abs() < 1e-9));
    }
}
