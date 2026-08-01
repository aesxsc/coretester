# CoreTester

CoreTester is a native, dependency-light CPU benchmark for Windows and Linux. It pins work to one logical CPU at a time, runs several very different workloads, samples the sensors the operating system exposes, and produces both a terminal summary and an interactive self-contained HTML report.

## What it measures

- Integer and floating-point throughput
- Memory streaming bandwidth
- Random-access memory latency
- The fastest supported SIMD/crypto/bit-manipulation kernels implemented for the current architecture
- Per-logical-CPU relative scores, consistency, best/worst CPUs, and physical-core/SMT topology where the OS exposes it
- Package temperature and energy/power on a best-effort basis

On x86/x86-64, acceleration kernels currently cover SSE2, AVX2, FMA, AES-NI, SSE4.2 CRC32, POPCNT, BMI1, BMI2, PCLMULQDQ, SHA-NI, ADX, RDRAND, and RDSEED when present. On AArch64, CoreTester detects architectural features and uses NEON for its accelerated floating-point path. Unsupported features are never executed.

## Build and run

```powershell
cargo build --release
.\target\release\coretester.exe
```

Linux:

```bash
cargo build --release
./target/release/coretester
```

Useful options:

```text
--quick                 Short validation run
--duration-ms N         Time per main kernel (default: 180 ms)
--cores LIST            CPUs such as 0,2-5,8
--output PATH           HTML report path
--json PATH             JSON result path
--no-open               Do not open the HTML report
--help                   Show all options
```

For the cleanest comparison, close heavy applications, connect AC power, select a fixed/high-performance power plan, let the machine cool first, and repeat the run. CoreTester tests logical CPUs sequentially to reduce cross-core contention. Laptop firmware, boost limits, background tasks, SMT siblings, and run order can still affect results.

## Sensor limitations

Linux power readings use the kernel powercap/RAPL energy counters when readable. Temperatures use thermal-zone and hwmon sysfs entries. These are package or board sensors, not reliable per-core probes, so the report labels them as sampled package values.

Stock Windows does not expose dependable package energy and temperature telemetry through a stable unprivileged API on most systems. CoreTester reports those fields as unavailable rather than inventing estimates. Performance and latency measurements remain fully functional. Vendor tools, firmware counters, or a hardware-monitor driver can be added later as a sensor backend.

"Every instruction set" is not literally attainable in one safe portable application: CPUs expose hundreds of instructions, some are privileged, stateful, workload-specific, or require operating-system enablement. CoreTester detects a broad feature inventory and executes every safe acceleration kernel it implements. The report distinguishes detected features from benchmarked acceleration paths.
