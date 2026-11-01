// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # `edgeforge model list|import|verify`
//!
//! ## Purpose
//! Manage imported models from the command line. The `import` command
//! computes a SHA-256 hash of the model file, detects the precision from
//! the filename, and inserts the model into the registry.
//!
//! ## What This File Does
//! - `list` — lists all models with ID, name, precision, size
//! - `import --path <p>` — imports a model (SHA-256 hash + registry insert)
//! - `verify --id <id>` — verifies a model exists in the registry
//!
//! ## Dependencies
//! - `edgeforge-orchestration::{Models, NewModelRow, ModelArch}` — ORM
//! - `edgeforge-core::{Config, Precision, ModelId}` — types
//! - `sqlx` — SQLite connection
//! - `ring::digest` — SHA-256 hashing
//! - `std::fs` — file metadata
//!
//! ## Output
//! Console output (table for list, status for verify, hash + ID for import).

use super::{ModelArgs, ModelCommand};
use edgeforge_core::Config;
use edgeforge_orchestration::{ModelArch, Models, NewModelRow};
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;

/// Executes the main logic of this function.
pub async fn run(args: ModelArgs, cfg: Config) -> anyhow::Result<()> {
    let db_path = cfg.paths.data_dir.join("edgeforge.db");
    if let Some(p) = db_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&db_url)
        .await?;
    sqlx::migrate!("../../resources/migrations")
        .run(&pool)
        .await?;
    let models = Models::new(pool);

    match args.command {
        ModelCommand::List => {
            let rows = models.list().await?;
            if rows.is_empty() {
                println!("No models imported yet.");
            } else {
                println!("{:-<80}", "");
                println!(
                    "{:<26} {:<20} {:<10} {:>12}",
                    "ID", "NAME", "PRECISION", "SIZE (GiB)"
                );
                println!("{:-<80}", "");
                let total = rows.len();
                for r in rows {
                    let gib = r.file_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    println!(
                        "{:<26} {:<20} {:<10} {:>12.2}",
                        r.id, r.name, r.precision, gib
                    );
                }
                println!("{:-<80}", "");
                println!("Total: {} model(s)", total);
            }
        }
        ModelCommand::Import { path } => {
            let model_path = Path::new(&path);
            anyhow::ensure!(model_path.exists(), "file not found: {path}");

            // Get file metadata.
            let metadata = std::fs::metadata(model_path)?;
            let file_size = metadata.len();
            let file_name = model_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            println!("Importing model: {file_name}");
            println!("  Path: {path}");
            println!(
                "  Size: {:.2} GiB ({} bytes)",
                file_size as f64 / (1024.0 * 1024.0 * 1024.0),
                file_size
            );

            // Compute SHA-256 hash.
            println!("  Computing SHA-256 hash...");
            let sha256 = compute_sha256(model_path)?;
            println!("  SHA-256: {}", hex::encode(&sha256));

            // Detect precision from filename.
            let precision = detect_precision(&file_name);
            println!("  Detected precision: {precision:?}");

            // Detect architecture from filename.
            let arch = detect_arch(&file_name);
            println!("  Detected architecture: {arch:?}");

            // Insert into the registry.
            let new_row = NewModelRow {
                name: file_name.clone(),
                publisher: "unknown".to_string(),
                license: "unknown".to_string(),
                file_path: model_path.to_path_buf(),
                file_size_bytes: file_size,
                sha256: sha256.to_vec(),
                precision,
                arch,
                total_params: 0, // Would be read from the GGUF header
                active_params: None,
                dense_core_bytes: 0,
                expert_pool_bytes: 0,
            };

            let model_id = models.insert(&new_row).await?;
            println!("\n  ✅ Model imported successfully!");
            println!("  Model ID: {model_id}");
            println!("  Use with: edgeforge run --model {model_id} --prompt \"Hello!\"");
        }
        ModelCommand::Verify { id } => {
            let mid = edgeforge_core::ModelId::parse(&id)
                .map_err(|e| anyhow::anyhow!("invalid model id: {e}"))?;
            match models.get(mid).await? {
                Some(r) => {
                    println!("✅ Model verified:");
                    println!("  ID:         {}", r.id);
                    println!("  Name:       {}", r.name);
                    println!("  Publisher:  {}", r.publisher);
                    println!("  License:    {}", r.license);
                    println!("  Precision:  {}", r.precision);
                    println!(
                        "  Size:       {:.2} GiB",
                        r.file_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    );
                    println!("  Imported:   {}", r.imported_at);
                }
                None => {
                    anyhow::bail!("model not found: {id}");
                }
            }
        }
    }
    Ok(())
}

/// Compute the SHA-256 hash of a file using streaming (1 MiB chunks).
///
/// ## Parameters
/// - `path`: Path to the file to hash
///
/// ## Returns
/// `Ok([u8; 32])` — the 32-byte SHA-256 hash.
///
/// ## Errors
/// Returns an error if the file cannot be read.
fn compute_sha256(path: &Path) -> anyhow::Result<[u8; 32]> {
    use ring::digest;
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
    let mut ctx = digest::Context::new(&digest::SHA256);
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    let digest = ctx.finish();
    let mut result = [0u8; 32];
    result.copy_from_slice(digest.as_ref());
    Ok(result)
}

/// Detect the precision from the model filename.
///
/// Looks for common quantization suffixes: Q4_K_M, Q8_0, FP16, F16, etc.
///
/// ## Parameters
/// - `filename`: The model filename (without extension)
///
/// ## Returns
/// The detected `Precision`, defaulting to `FP16`.
fn detect_precision(filename: &str) -> edgeforge_core::Precision {
    let lower = filename.to_lowercase();
    if lower.contains("q3") {
        edgeforge_core::Precision::Q3
    } else if lower.contains("q4") {
        edgeforge_core::Precision::Q4
    } else if lower.contains("q8") {
        edgeforge_core::Precision::Q8
    } else if lower.contains("bf16") {
        edgeforge_core::Precision::Bf16
    } else if lower.contains("fp32") || lower.contains("f32") {
        edgeforge_core::Precision::Fp32
    } else {
        edgeforge_core::Precision::Fp16 // default
    }
}

/// Detect the model architecture from the filename.
///
/// Looks for common architecture keywords: moe, mixtral, dense, rwkv, mamba.
///
/// ## Parameters
/// - `filename`: The model filename (without extension)
///
/// ## Returns
/// The detected `ModelArch`, defaulting to `Dense`.
fn detect_arch(filename: &str) -> ModelArch {
    let lower = filename.to_lowercase();
    if lower.contains("moe") || lower.contains("mixtral") || lower.contains("deepseek") {
        ModelArch::Moe
    } else if lower.contains("rwkv") {
        ModelArch::Rwkv
    } else if lower.contains("mamba") {
        ModelArch::Mamba
    } else {
        ModelArch::Dense
    }
}
