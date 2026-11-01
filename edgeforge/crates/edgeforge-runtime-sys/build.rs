// edgeforge-runtime-sys/build.rs
//
// Builds llama.cpp as a static library and generates Rust FFI bindings via bindgen.
//
// This build script:
// 1. Checks if ffi/llama.cpp/ exists (git submodule).
// 2. If present: builds via cmake, generates bindings via bindgen.
// 3. If absent: compiles a stub that returns NotImplemented errors.
//
// The `llama-backend` feature flag on edgeforge-runtime controls whether
// the real bindings are expected. Without the feature, the build always
// uses the stub path.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    // The llama.cpp source directory relative to the workspace root.
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("could not find workspace root");
    let llama_dir = workspace_root.join("ffi").join("llama.cpp");
    let llama_build_dir = llama_dir.join("build");

    // Check if the "llama-backend" feature is enabled.
    let llama_backend_feature = std::env::var("CARGO_FEATURE_LLAMA_BACKEND").is_ok();

    if !llama_backend_feature {
        // Stub mode — no llama.cpp needed.
        println!("cargo:rustc-cfg=llama_stub");
        println!("cargo:rerun-if-changed=wrapper.h");
        return;
    }

    // Real mode — check if llama.cpp source is present.
    if !llama_dir.exists() || !llama_dir.join("CMakeLists.txt").exists() {
        eprintln!("warning: llama-backend feature is enabled but ffi/llama.cpp/ is not present.");
        eprintln!("  Run: git submodule update --init --recursive");
        eprintln!("  Falling back to stub mode.");
        println!("cargo:rustc-cfg=llama_stub");
        println!("cargo:rerun-if-changed=wrapper.h");
        return;
    }

    // ── Step 1: Build llama.cpp via CMake ──────────────────────────────
    //
    // Detects the host triple and CPU ISA, then configures and builds
    // llama.cpp as a static library.

    if !llama_build_dir.exists() {
        std::fs::create_dir_all(&llama_build_dir).expect("failed to create build dir");
    }

    // CMake configure.
    let mut cmake_cmd = Command::new("cmake");
    cmake_cmd
        .current_dir(&llama_build_dir)
        .arg(&llama_dir)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DLLAMA_STATIC=ON",
            "-DLLAMA_SHARED=OFF",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            "-DLLAMA_BUILD_SERVER=OFF",
        ]);

    // CPU ISA detection (matches scripts/build-llama.sh logic).
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            cmake_cmd.args(["-DLLAMA_AVX512=ON", "-DLLAMA_FMA=ON", "-DLLAMA_F16C=ON"]);
        } else if is_x86_feature_detected!("avx2") {
            cmake_cmd.args(["-DLLAMA_AVX2=ON", "-DLLAMA_FMA=ON", "-DLLAMA_F16C=ON"]);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        cmake_cmd.args(["-DLLAMA_NEON=ON"]);
    }

    let status = cmake_cmd
        .status()
        .expect("failed to run cmake — is cmake installed?");
    assert!(status.success(), "cmake configure failed");

    // CMake build.
    let nproc = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let status = Command::new("cmake")
        .current_dir(&llama_build_dir)
        .args(["--build", ".", "--config", "Release", "-j", &nproc.to_string()])
        .status()
        .expect("failed to run cmake build");
    assert!(status.success(), "cmake build failed");

    // Link the static library.
    println!("cargo:rustc-link-search=native={}", llama_build_dir.display());
    println!("cargo:rustc-link-lib=static=llama");
    println!("cargo:rustc-link-lib=stdc++");

    // ── Step 2: Generate bindings via bindgen ──────────────────────────
    //
    // Generates Rust FFI bindings from wrapper.h, which includes the
    // llama.cpp C API headers.

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .allowlist_function("llama_.*")
        .allowlist_type("llama_.*")
        .allowlist_var("llama_.*")
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .ctypes_prefix("::std::os::raw")
        .generate()
        .expect("failed to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings");

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=ffi/llama.cpp/CMakeLists.txt");
}

// CPU feature detection helper (x86_64 only).
#[cfg(target_arch = "x86_64")]
fn is_x86_feature_detected(feature: &str) -> bool {
    match feature {
        "avx512f" => std::is_x86_feature_detected!("avx512f"),
        "avx2" => std::is_x86_feature_detected!("avx2"),
        _ => false,
    }
}
