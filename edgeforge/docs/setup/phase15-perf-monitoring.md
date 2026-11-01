# EdgeForge Phase 15 — Performance Monitoring Module Setup Guide

## Overview

Phase 15 introduces the `edgeforge-perf` crate — a comprehensive performance monitoring, benchmark metrics, and comparative benchmarking module.

## New Crate: `edgeforge-perf`

### Modules

| Module | Purpose |
|---|---|
| `hardware.rs` | Hardware specification profiler (CPU, RAM, SSD, GPU) |
| `function_perf.rs` | Per-function KPI tracking with hardware attribution |
| `comparative.rs` | Comparative benchmark registry for known platforms |
| `monitor.rs` | Real-time performance monitor with regression detection |

### Quick Start

```rust
use edgeforge_perf::{profile_hardware, FunctionPerfTracker, ComparativeRegistry, PerfMonitor};

// 1. Profile the current hardware
let hw = profile_hardware().unwrap();
println!("CPU: {} ({} cores)", hw.cpu.brand, hw.cpu.physical_cores);
println!("RAM: {:.1} GiB", hw.ram.total_gib);
println!("Signature: {}", hw.signature);

// 2. Track function performance
let tracker = FunctionPerfTracker::new();
tracker.record_call("my-crate", "file.rs", "my_fn", FunctionKpi::success(1500), None);
let stats = tracker.get_stats("my_fn").unwrap();
println!("Mean: {:.0}µs, p99: {}µs", stats.mean_duration_us, stats.p99_duration_us);

// 3. Compare across platforms
let registry = ComparativeRegistry::with_known_platforms();
let platform_a = &registry.get_platforms()[0];
let platform_b = &registry.get_platforms()[1];
if let Some(comparison) = registry.compare("load", platform_a, platform_b) {
    println!("Speedup: {:.2}x ({:?})", comparison.speedup_ratio, comparison.verdict);
}

// 4. Monitor for regressions
let monitor = PerfMonitor::new(
    PerfMonitorConfig::default(),
    std::sync::Arc::new(tracker),
    std::sync::Arc::new(registry),
).unwrap();
monitor.set_baseline();
// ... run workload ...
let alerts = monitor.check_regression();
for alert in &alerts {
    println!("REGRESSION: {} +{:.0}%", alert.function_name, alert.percent_increase);
}
```

### PerfGuard Macro

Use `PerfGuard` to automatically track function performance:

```rust
use edgeforge_perf::function_perf::PerfGuard;

pub fn my_function() -> Result<(), Error> {
    let _guard = PerfGuard::new("edgeforge-runtime", "engine.rs", "generate");
    // ... function body ...
    Ok(()) // guard records success on drop
}
```

### Hardware Spec Output

```json
{
  "cpu": {
    "brand": "AMD Ryzen 9 7950X",
    "physical_cores": 16,
    "logical_cores": 32,
    "architecture": "x86_64",
    "isa_extensions": ["avx2", "fma", "sse4.2"]
  },
  "ram": {
    "total_bytes": 68719476736,
    "total_gib": 64.0,
    "available_bytes": 34359738368
  },
  "ssd": {
    "model": "Samsung 990 Pro 4TB",
    "total_gib": 3726.0,
    "interface": "NVMe"
  },
  "os": "Linux",
  "signature": "amd-ryzen-9-7950x_64gb_linux"
}
```

### Comparative Benchmark Output

```json
{
  "function_name": "generate",
  "platform_a": "ryzen9-7950x_64gb_linux",
  "platform_b": "apple-m2-max_64gb_macos",
  "mean_a_us": 2000.0,
  "mean_b_us": 1800.0,
  "speedup_ratio": 1.11,
  "percent_difference": -10.0,
  "verdict": "faster"
}
```

### Regression Alert Output

```
REGRESSION: generate — baseline p99=5000µs, current p99=8000µs (+60%, major)
REGRESSION: page_in — baseline p99=1500µs, current p99=2200µs (+46%, minor)
```

## Known Reference Platforms

| Platform | CPU | RAM | OS |
|---|---|---|---|
| Apple M2 Max | M2 Max | 64 GB Unified | macOS |
| AMD Ryzen 9 7950X | 16c/32t | 64 GB DDR5 | Linux |
| Intel Core i9-13900K | 24c/32t | 32 GB DDR5 | Linux |
| AMD Ryzen 9 5900X | 12c/24t | 32 GB DDR4 | Linux |
| Apple M1 Pro | M1 Pro | 16 GB Unified | macOS |

## Build

```bash
# Build with performance monitoring (included by default)
cargo build --release --workspace

# Run tests
cargo test --workspace

# Run perf module tests only
cargo test -p edgeforge-perf
```
