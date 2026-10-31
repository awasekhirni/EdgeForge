// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::db::now_ms;
use edgeforge_core::{EdgeForgeError, JobId, ModelId};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
pub struct Jobs {
    pool: SqlitePool,
}
impl Jobs {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    pub async fn enqueue(&self, req: EnqueueJob) -> Result<JobId, EdgeForgeError> {
        let id = JobId::new();
        let now = now_ms();
        sqlx::query("INSERT INTO jobs (id,model_id,prompt_hash,status,preset,priority,params_json,tokens_generated,created_at) VALUES (?,?,?,'queued',?,?,?,0,?)")
            .bind(id.to_string()).bind(req.model_id.to_string()).bind(req.prompt_hash).bind(req.preset).bind(req.priority.as_str()).bind(req.params_json).bind(now)
            .execute(&self.pool).await.map_err(crate::db_err)?;
        Ok(id)
    }
    pub async fn pick_next(&self) -> Result<Option<JobRow>, EdgeForgeError> {
        Ok(sqlx::query_as::<_, JobRow>("SELECT * FROM jobs WHERE status='queued' ORDER BY CASE priority WHEN 'interactive' THEN 0 ELSE 1 END, created_at ASC LIMIT 1").fetch_optional(&self.pool).await.map_err(crate::db_err)?)
    }
    pub async fn mark_done(&self, id: JobId, tokens: u32) -> Result<(), EdgeForgeError> {
        sqlx::query("UPDATE jobs SET status='done', tokens_generated=?, finished_at=? WHERE id=? AND status='running'").bind(tokens as i64).bind(now_ms()).bind(id.to_string()).execute(&self.pool).await.map_err(crate::db_err)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct EnqueueJob {
    pub model_id: ModelId,
    pub prompt_hash: Vec<u8>,
    pub preset: String,
    pub priority: JobPriority,
    pub params_json: String,
}
