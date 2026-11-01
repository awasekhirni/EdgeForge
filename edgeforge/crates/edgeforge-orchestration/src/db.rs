// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # ORM Models — Model Registry (F-LOAD-01)
//!
//! ## Purpose
//! Typed access to the `models` table. Provides CRUD operations for the model
//! registry using sqlx::query_as.
//!
//! ## What This File Does
//! - `ModelRow` — ORM row for the models table (FromRow)
//! - `Models` — typed access struct owning a SqlitePool
//! - `insert(row)` — insert a new model, returns ModelId
//! - `list()` — list all models, most-recently-used first
//! - `get(id)` — fetch a single model by ID
//! - `delete(id)` — delete a model from the registry
//! - `NewModelRow` — payload for insert
//! - `ModelArch` — MoE/Dense/RWKV/Mamba
//! - `now_ms()` — current unix timestamp in milliseconds
//!
//! ## Dependencies
//! - `sqlx::{SqlitePool, FromRow}` — SQLite ORM
//! - `edgeforge-core::{EdgeForgeError, ModelId, Precision}`
//! - `serde` — for ModelRow serialization
//!
//! ## Output
//! `Result<ModelId, EdgeForgeError>` from insert;
//! `Result<Vec<ModelRow>, EdgeForgeError>` from list;
//! `Result<Option<ModelRow>, EdgeForgeError>` from get.
//!
//! ## Function: insert
//! - **Purpose:** Insert a new model into the registry
//! - **Parameters:** `&NewModelRow` — model metadata (name, publisher, license, file info, precision, arch)
//! - **Returns:** `Result<ModelId, EdgeForgeError>` — the new model's ULID
//! - **Errors:** Database error if INSERT fails (e.g., constraint violation)
//! - **Dependencies:** `sqlx::query` with bind parameters; `crate::db_err` for error conversion
//! - **Side effects:** Inserts a row with `imported_at = now_ms()`, `last_used_at = NULL`

use edgeforge_core::{EdgeForgeError, ModelId, Precision};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
use std::path::PathBuf;
#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
/// `ModelRow` — see module documentation for purpose and usage.
pub struct ModelRow {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub license: String,
    pub file_path: String,
    pub file_size_bytes: i64,
    pub sha256: Vec<u8>,
    pub precision: String,
    pub arch: String,
    pub total_params: i64,
    pub active_params: Option<i64>,
    pub dense_core_bytes: i64,
    pub expert_pool_bytes: i64,
    pub imported_at: i64,
    pub last_used_at: Option<i64>,
}
impl ModelRow {
    /// Id — see module documentation for details.
    pub fn id(&self) -> Result<ModelId, EdgeForgeError> {
        ModelId::parse(&self.id).map_err(|e| EdgeForgeError::Database(e.to_string()))
    }
    /// Precision — see module documentation for details.
    pub fn precision(&self) -> Result<Precision, EdgeForgeError> {
        Ok(match self.precision.as_str() {
            "fp32" => Precision::Fp32,
            "fp16" => Precision::Fp16,
            "bf16" => Precision::Bf16,
            "q8" => Precision::Q8,
            "q4" => Precision::Q4,
            "q3" => Precision::Q3,
            o => return Err(EdgeForgeError::Database(format!("unknown precision: {o}"))),
        })
    }
}
/// `Models` — see module documentation for purpose and usage.
pub struct Models {
    pool: SqlitePool,
}
impl Models {
    /// Creates a new instance with default configuration.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    /// Inserts a new entry.
    pub async fn insert(&self, row: &NewModelRow) -> Result<ModelId, EdgeForgeError> {
        let id = ModelId::new();
        let now = now_ms();
        sqlx::query("INSERT INTO models (id,name,publisher,license,file_path,file_size_bytes,sha256,precision,arch,total_params,active_params,dense_core_bytes,expert_pool_bytes,imported_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(id.to_string()).bind(&row.name).bind(&row.publisher).bind(&row.license).bind(row.file_path.to_string_lossy().to_string()).bind(row.file_size_bytes as i64).bind(&row.sha256).bind(prec_str(row.precision)).bind(arch_str(row.arch)).bind(row.total_params as i64).bind(row.active_params.map(|v| v as i64)).bind(row.dense_core_bytes as i64).bind(row.expert_pool_bytes as i64).bind(now)
            .execute(&self.pool).await.map_err(crate::db_err)?;
        Ok(id)
    }
    /// Lists all entries.
    pub async fn list(&self) -> Result<Vec<ModelRow>, EdgeForgeError> {
        Ok(sqlx::query_as::<_, ModelRow>(
            "SELECT * FROM models ORDER BY COALESCE(last_used_at, imported_at) DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?)
    }
    /// Retrieves an item from the cache.
    pub async fn get(&self, id: ModelId) -> Result<Option<ModelRow>, EdgeForgeError> {
        Ok(
            sqlx::query_as::<_, ModelRow>("SELECT * FROM models WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(crate::db_err)?,
        )
    }
    /// Deletes an entry by ID.
    pub async fn delete(&self, id: ModelId) -> Result<(), EdgeForgeError> {
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(crate::db_err)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
/// `NewModelRow` — see module documentation for purpose and usage.
pub struct NewModelRow {
    pub name: String,
    pub publisher: String,
    pub license: String,
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub sha256: Vec<u8>,
    pub precision: Precision,
    pub arch: ModelArch,
    pub total_params: u64,
    pub active_params: Option<u64>,
    pub dense_core_bytes: u64,
    pub expert_pool_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelArch {
    Moe,
    Dense,
    Rwkv,
    Mamba,
}
fn prec_str(p: Precision) -> &'static str {
    match p {
        Precision::Fp32 => "fp32",
        Precision::Fp16 => "fp16",
        Precision::Bf16 => "bf16",
        Precision::Q8 => "q8",
        Precision::Q4 => "q4",
        Precision::Q3 => "q3",
    }
}
fn arch_str(a: ModelArch) -> &'static str {
    match a {
        ModelArch::Moe => "moe",
        ModelArch::Dense => "dense",
        ModelArch::Rwkv => "rwkv",
        ModelArch::Mamba => "mamba",
    }
}
/// Returns the current Unix timestamp in milliseconds.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
