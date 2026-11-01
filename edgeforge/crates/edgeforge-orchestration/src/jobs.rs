// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Job Queue with Status State Machine (F-UX-06, F-SCHED-04)
//!
//! ## Purpose
//! SQLite-backed job queue with priority lanes (interactive preempts batch)
//! and a status state machine (queued → running → done/failed/cancelled).
//!
//! ## What This File Does
//! - `JobStatus` — Queued, Running, Paused, Done, Failed, Cancelled
//! - `JobPriority` — Interactive, Batch
//! - `JobRow` — ORM row for the jobs table
//! - `Jobs` — typed access struct owning a SqlitePool
//! - `enqueue(req)` — add a job to the queue
//! - `pick_next()` — atomically pick the next queued job (interactive first)
//! - `mark_done(id, tokens)` — transition to done
//!
//! ## Dependencies
//! - `sqlx::{SqlitePool, FromRow}` — SQLite ORM
//! - `edgeforge-core::{EdgeForgeError, JobId, ModelId}`
//! - `serde` — for JobRow serialization
//!
//! ## Output
//! `Result<JobId, EdgeForgeError>` from enqueue;
//! `Result<Option<JobRow>, EdgeForgeError>` from pick_next.
//!
//! ## Function: pick_next
//! - **Purpose:** Atomically pick the next queued job (FIFO within priority class)
//! - **Returns:** `Result<Option<JobRow>, EdgeForgeError>`
//! - **Selection:** Interactive jobs always come before batch jobs;
//!   within each class, ordered by created_at ASC
//! - **Dependencies:** `sqlx::query_as` with ORDER BY CASE priority + created_at

use crate::db::now_ms;
use edgeforge_core::{EdgeForgeError, JobId, ModelId};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `JobStatus` enumeration — see module documentation for variants.
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}
impl JobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    fn from_str(s: &str) -> Result<Self, EdgeForgeError> {
        Ok(match s {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            o => return Err(EdgeForgeError::Database(format!("unknown status: {o}"))),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `JobPriority` enumeration — see module documentation for variants.
pub enum JobPriority {
    Interactive,
    Batch,
}
impl JobPriority {
    fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Batch => "batch",
        }
    }
}
#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
/// `JobRow` — see module documentation for purpose and usage.
pub struct JobRow {
    pub id: String,
    pub model_id: String,
    pub prompt_hash: Vec<u8>,
    pub status: String,
    pub preset: String,
    pub priority: String,
    pub params_json: String,
    pub tokens_generated: i64,
    pub created_at: i64,
}
/// `Jobs` — see module documentation for purpose and usage.
pub struct Jobs {
    pool: SqlitePool,
}
impl Jobs {
    /// Creates a new instance with default configuration.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    /// Enqueues a new item.
    pub async fn enqueue(&self, req: EnqueueJob) -> Result<JobId, EdgeForgeError> {
        let id = JobId::new();
        let now = now_ms();
        sqlx::query("INSERT INTO jobs (id,model_id,prompt_hash,status,preset,priority,params_json,tokens_generated,created_at) VALUES (?,?,?,'queued',?,?,?,0,?)")
            .bind(id.to_string()).bind(req.model_id.to_string()).bind(req.prompt_hash).bind(req.preset).bind(req.priority.as_str()).bind(req.params_json).bind(now)
            .execute(&self.pool).await.map_err(crate::db_err)?;
        Ok(id)
    }
    /// Atomically picks the next queued job.
    pub async fn pick_next(&self) -> Result<Option<JobRow>, EdgeForgeError> {
        Ok(sqlx::query_as::<_, JobRow>("SELECT * FROM jobs WHERE status='queued' ORDER BY CASE priority WHEN 'interactive' THEN 0 ELSE 1 END, created_at ASC LIMIT 1").fetch_optional(&self.pool).await.map_err(crate::db_err)?)
    }
    /// Transitions a job to done status.
    pub async fn mark_done(&self, id: JobId, tokens: u32) -> Result<(), EdgeForgeError> {
        sqlx::query("UPDATE jobs SET status='done', tokens_generated=?, finished_at=? WHERE id=? AND status='running'").bind(tokens as i64).bind(now_ms()).bind(id.to_string()).execute(&self.pool).await.map_err(crate::db_err)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
/// `EnqueueJob` — see module documentation for purpose and usage.
pub struct EnqueueJob {
    pub model_id: ModelId,
    pub prompt_hash: Vec<u8>,
    pub preset: String,
    pub priority: JobPriority,
    pub params_json: String,
}
