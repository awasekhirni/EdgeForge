// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use super::{ModelArgs, ModelCommand};
use edgeforge_core::Config;
use edgeforge_orchestration::Models;
use sqlx::sqlite::SqlitePoolOptions;
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
                for r in rows {
                    let gib = r.file_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    println!(
                        "{:<26} {:<20} {:<10} {:>10.2} GiB",
                        r.id, r.name, r.precision, gib
                    );
                }
            }
        }
        ModelCommand::Import { path } => {
            println!("import: stub — Phase 3 will implement manifest verify + hash");
            println!("  path: {path}");
        }
        ModelCommand::Verify { id } => {
            let mid = edgeforge_core::ModelId::parse(&id)
                .map_err(|e| anyhow::anyhow!("invalid id: {e}"))?;
            match models.get(mid).await? {
                Some(r) => println!("OK   id={} name={} precision={}", r.id, r.name, r.precision),
                None => anyhow::bail!("model not found: {id}"),
            }
        }
    }
    Ok(())
}
