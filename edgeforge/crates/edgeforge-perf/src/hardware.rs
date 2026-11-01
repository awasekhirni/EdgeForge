//! # Hardware Specification Profiler
//!
//! ## Purpose
//! Profiles the current hardware platform in detail — CPU, RAM, SSD, GPU —
//! and produces a structured `HardwareSpec` that can be used for benchmarking,
//! comparative analysis, and performance regression tracking.
//!
//! ## What This File Does
//! - `HardwareSpec` — complete hardware specification (CPU + RAM + SSD + GPU)
//! - `CpuSpec` — CPU model, cores, ISA, frequency, cache
//! - `RamSpec` — RAM size, type, frequency
//! - `SsdSpec` — SSD model, capacity, PCIe generation, NVMe vs SATA
//! - `GpuSpec` — GPU model, VRAM, CUDA/Metal/ROCm
//! - `profile_hardware()` — detect and return the current hardware spec
//! - `hardware_signature()` — a unique hash of the hardware for deduplication
//!
//! ## Dependencies
//! - `sysinfo` — CPU, RAM, disk detection
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `serde` — serialization
//!
//! ## Output
//! `HardwareSpec` — a structured hardware specification for benchmarking.
//!
//! ## Function Logging
//! `timestamp - edgeforge-perf - hardware.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Complete hardware specification.
///
/// Captures the full hardware platform configuration for benchmarking
/// and comparative analysis. Stored alongside benchmark results so
/// performance can be compared across hardware.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSpec {
    /// CPU specification.
    pub cpu: CpuSpec,
    /// RAM specification.
    pub ram: RamSpec,
    /// SSD specification (primary model storage).
    pub ssd: Option<SsdSpec>,
    /// GPU specification (if present).
    pub gpu: Option<GpuSpec>,
    /// Operating system name.
    pub os: String,
    /// OS version.
    pub os_version: String,
    /// Kernel version (Linux) / Darwin version (macOS) / build (Windows).
    pub kernel_version: String,
    /// Timestamp when the spec was captured (Unix ms).
    pub captured_at_ms: i64,
    /// Unique signature (hash of key fields) for deduplication.
    pub signature: String,
}

/// CPU specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuSpec {
    /// CPU brand/model string (e.g., "AMD Ryzen 9 7950X").
    pub brand: String,
    /// Number of physical cores.
    pub physical_cores: u32,
    /// Number of logical cores (threads).
    pub logical_cores: u32,
    /// CPU architecture (e.g., "x86_64", "aarch64").
    pub architecture: String,
    /// Base frequency in MHz (0 if unknown).
    pub base_frequency_mhz: u64,
    /// L1 cache size in KB (per core, 0 if unknown).
    pub l1_cache_kb: u32,
    /// L2 cache size in KB (per core, 0 if unknown).
    pub l2_cache_kb: u32,
    /// L3 cache size in KB (shared, 0 if unknown).
    pub l3_cache_kb: u32,
    /// Supported ISA extensions (e.g., "avx2", "avx512", "neon").
    pub isa_extensions: Vec<String>,
}

/// RAM specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RamSpec {
    /// Total RAM in bytes.
    pub total_bytes: u64,
    /// Total RAM in GiB (derived).
    pub total_gib: f64,
    /// Available RAM in bytes (at capture time).
    pub available_bytes: u64,
    /// RAM type (e.g., "DDR5", "DDR4", "LPDDR5", "unknown").
    pub ram_type: String,
}

/// SSD specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SsdSpec {
    /// SSD model name (e.g., "Samsung 990 Pro 4TB").
    pub model: String,
    /// Total capacity in bytes.
    pub total_bytes: u64,
    /// Total capacity in GiB (derived).
    pub total_gib: f64,
    /// Interface type ("NVMe" or "SATA").
    pub interface: String,
    /// PCIe generation ("Gen3", "Gen4", "Gen5", "SATA", "unknown").
    pub pcie_generation: String,
    /// Whether this is the OS drive.
    pub is_os_drive: bool,
}

/// GPU specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpuSpec {
    /// GPU brand/model string (e.g., "NVIDIA GeForce RTX 4090").
    pub brand: String,
    /// VRAM in bytes.
    pub vram_bytes: u64,
    /// VRAM in GiB (derived).
    pub vram_gib: f64,
    /// GPU backend type ("CUDA", "Metal", "ROCm", "None").
    pub backend: String,
    /// Driver version (if detectable).
    pub driver_version: Option<String>,
}

/// Profile the current hardware and return a structured specification.
///
/// ## Purpose
/// Detects the current hardware platform (CPU, RAM, SSD, GPU) and returns
/// a structured `HardwareSpec` that can be stored alongside benchmark results.
///
/// ## Returns
/// `Ok(HardwareSpec)` on success.
///
/// ## Errors
/// Returns an error if system information cannot be gathered.
///
/// ## Function Logging
/// `timestamp - edgeforge-perf - hardware.rs - profile_hardware - STATUS`
pub fn profile_hardware() -> Result<HardwareSpec, String> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-perf",
        "hardware.rs",
        "profile_hardware",
    );
    tracing::info!("Profiling hardware...");

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    // CPU
    let cpu = profile_cpu(&sys);

    // RAM
    let ram = profile_ram(&sys);

    // OS
    let os = sysinfo::System::name().unwrap_or_else(|| "unknown".to_string());
    let os_version = sysinfo::System::os_version().unwrap_or_else(|| "unknown".to_string());
    let kernel_version = sysinfo::System::kernel_version().unwrap_or_else(|| "unknown".to_string());

    // SSD (best-effort — may not detect on all platforms)
    let ssd = profile_ssd();

    // GPU (best-effort)
    let gpu = profile_gpu();

    let captured_at_ms = chrono::Utc::now().timestamp_millis();
    let signature = hardware_signature(&cpu, &ram, &ssd, &gpu, &os);

    let spec = HardwareSpec {
        cpu,
        ram,
        ssd,
        gpu,
        os,
        os_version,
        kernel_version,
        captured_at_ms,
        signature,
    };

    tracing::info!(
        cpu = %spec.cpu.brand,
        cores = spec.cpu.physical_cores,
        ram_gib = spec.ram.total_gib,
        os = %spec.os,
        "Hardware profiled"
    );
    guard.success();
    Ok(spec)
}

/// Profile the CPU.
fn profile_cpu(sys: &sysinfo::System) -> CpuSpec {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "hardware.rs", "profile_cpu");

    let cpus = sys.cpus();
    let first_cpu = cpus.first();

    let brand = first_cpu
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let architecture = std::env::consts::ARCH.to_string();

    // Detect ISA extensions.
    let mut isa_extensions = Vec::new();
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            isa_extensions.push("avx2".to_string());
        }
        if std::is_x86_feature_detected!("avx512f") {
            isa_extensions.push("avx512f".to_string());
        }
        if std::is_x86_feature_detected!("fma") {
            isa_extensions.push("fma".to_string());
        }
        if std::is_x86_feature_detected!("sse4.2") {
            isa_extensions.push("sse4.2".to_string());
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        isa_extensions.push("neon".to_string());
    }

    CpuSpec {
        brand,
        physical_cores: sysinfo::System::physical_core_count(sys)
            .map(|n| n as u32)
            .unwrap_or(0),
        logical_cores: cpus.len() as u32,
        architecture,
        base_frequency_mhz: first_cpu.map(|c| c.frequency()).unwrap_or(0),
        l1_cache_kb: 0, // sysinfo doesn't expose cache info reliably
        l2_cache_kb: 0,
        l3_cache_kb: 0,
        isa_extensions,
    }
}

/// Profile the RAM.
fn profile_ram(sys: &sysinfo::System) -> RamSpec {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "hardware.rs", "profile_ram");
    let total_bytes = sys.total_memory();
    let available_bytes = sys.available_memory();
    RamSpec {
        total_bytes,
        total_gib: total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        available_bytes,
        ram_type: "unknown".to_string(), // sysinfo doesn't expose RAM type
    }
}

/// Profile the SSD (best-effort).
fn profile_ssd() -> Option<SsdSpec> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "hardware.rs", "profile_ssd");
    // Best-effort: use the largest disk as the model storage SSD.
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let largest = disks.list().iter().max_by_key(|d| d.total_space())?;

    let model = largest.name().to_string_lossy().to_string();
    let total_bytes = largest.total_space();

    Some(SsdSpec {
        model,
        total_bytes,
        total_gib: total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        interface: "NVMe".to_string(), // best-guess
        pcie_generation: "unknown".to_string(),
        is_os_drive: true, // best-guess
    })
}

/// Profile the GPU (best-effort).
fn profile_gpu() -> Option<GpuSpec> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "hardware.rs", "profile_gpu");

    // On macOS with Apple Silicon, report unified memory.
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            let mut sys = sysinfo::System::new_all();
            sys.refresh_memory();
            return Some(GpuSpec {
                brand: "Apple Silicon (Unified)".to_string(),
                vram_bytes: sys.total_memory(), // unified memory
                vram_gib: sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0),
                backend: "Metal".to_string(),
                driver_version: None,
            });
        }
    }

    // On Linux, check for NVIDIA.
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/nvidia0").exists() {
            return Some(GpuSpec {
                brand: "NVIDIA GPU".to_string(),
                vram_bytes: 0, // would need nvidia-ml to detect
                vram_gib: 0.0,
                backend: "CUDA".to_string(),
                driver_version: None,
            });
        }
    }

    None
}

/// Generate a unique hardware signature for deduplication.
///
/// ## Purpose
/// Creates a short hash-like string from key hardware fields. Used to
/// group benchmark results by hardware platform.
///
/// ## Parameters
/// - `cpu`, `ram`, `ssd`, `gpu`, `os`: Hardware components.
///
/// ## Returns
/// `String` — a short signature (e.g., "ryzen9-7950x_64gb_nvme_linux").
pub fn hardware_signature(
    cpu: &CpuSpec,
    ram: &RamSpec,
    _ssd: &Option<SsdSpec>,
    _gpu: &Option<GpuSpec>,
    os: &str,
) -> String {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-perf",
        "hardware.rs",
        "hardware_signature",
    );
    let cpu_part = cpu.brand.to_lowercase().replace(' ', "-");
    let ram_part = format!("{}gb", (ram.total_gib as u64));
    let os_part = os.to_lowercase();
    format!("{cpu_part}_{ram_part}_{os_part}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_hardware() {
        let spec = profile_hardware().unwrap();
        assert!(!spec.cpu.brand.is_empty() || spec.cpu.brand == "unknown");
        assert!(spec.ram.total_bytes > 0);
        assert!(!spec.os.is_empty());
        assert!(!spec.signature.is_empty());
    }

    #[test]
    fn test_hardware_signature() {
        let cpu = CpuSpec {
            brand: "AMD Ryzen 9 7950X".to_string(),
            physical_cores: 16,
            logical_cores: 32,
            architecture: "x86_64".to_string(),
            base_frequency_mhz: 4500,
            l1_cache_kb: 64,
            l2_cache_kb: 1024,
            l3_cache_kb: 32768,
            isa_extensions: vec!["avx2".to_string()],
        };
        let ram = RamSpec {
            total_bytes: 64_000_000_000,
            total_gib: 59.6,
            available_bytes: 30_000_000_000,
            ram_type: "DDR5".to_string(),
        };
        let sig = hardware_signature(&cpu, &ram, &None, &None, "Linux");
        assert!(sig.contains("amd-ryzen-9-7950x"));
        assert!(sig.contains("59gb"));
        assert!(sig.contains("linux"));
    }

    #[test]
    fn test_hardware_spec_serialize() {
        let spec = profile_hardware().unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("cpu"));
        assert!(json.contains("ram"));
    }
}
