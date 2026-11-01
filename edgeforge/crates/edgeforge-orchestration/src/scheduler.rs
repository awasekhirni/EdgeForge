// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed

//! # Priority-Lane Scheduler (F-SCHED-04)
//!
//! ## Purpose
//! Two-queue scheduler: interactive (preempts at layer boundary) and batch.
//! Interactive jobs always come before batch jobs.
//!
//! ## What This File Does
//! - `Scheduler` — owns interactive + batch VecDeques
//! - `enqueue(job, priority)` — add a job to the appropriate lane
//! - `pick()` — pick the next job (interactive first, then batch)
//!
//! ## Dependencies
//! - `edgeforge-core::JobId`
//! - `crate::JobPriority`
//! - `std::collections::VecDeque`
//!
//! ## Output
//! `Option<JobId>` — the next job to run, or None if both queues are empty.

use edgeforge_core::JobId;
use std::collections::VecDeque;
/// `Scheduler` — see module documentation for purpose and usage.
pub struct Scheduler {
    interactive: VecDeque<JobId>,
    batch: VecDeque<JobId>,
}
impl Scheduler {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            interactive: VecDeque::new(),
            batch: VecDeque::new(),
        }
    }
    /// Enqueues a new item.
    pub fn enqueue(&mut self, job: JobId, priority: crate::JobPriority) {
        match priority {
            crate::JobPriority::Interactive => self.interactive.push_back(job),
            crate::JobPriority::Batch => self.batch.push_back(job),
        }
    }
    /// Picks the next item.
    pub fn pick(&mut self) -> Option<JobId> {
        self.interactive
            .pop_front()
            .or_else(|| self.batch.pop_front())
    }
}
impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
