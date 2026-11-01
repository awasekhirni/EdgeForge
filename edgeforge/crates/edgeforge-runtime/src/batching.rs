// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Continuous Batching (F-SCHED-03)
//!
//! ## Purpose
//! Improves tok/s on multi-user nodes by batching in-flight requests.
//! Inspired by vLLM's continuous batching.
//!
//! ## What This File Does
//! - `BatchSlot` — holds one in-flight request (job + token count)
//! - `ContinuousBatcher` — manages multiple concurrent generation slots
//! - `admit(job)` — try to add a new request to the batch
//! - `release(job)` — remove a completed request
//! - `is_full()` / `len()` / `is_empty()` — status queries
//!
//! ## Dependencies
//! - `edgeforge_core::JobId`
//! - `std::collections::VecDeque`
//!
//! ## Output
//! `bool` from admit (true if admitted), `Option<JobId>` from next().
//!
//! ## Function: admit
//! - **Purpose:** Try to admit a new request into the batch
//! - **Parameters:** `JobId` — the job to admit
//! - **Returns:** `bool` — true if admitted, false if batch is full
//! - **Side effects:** Adds the job to an internal slot

use edgeforge_core::JobId;
use std::collections::VecDeque;

/// A batch slot holding one in-flight request.
pub struct BatchSlot {
    pub job: JobId,
    pub tokens_generated: u32,
}

/// The continuous batching scheduler.
pub struct ContinuousBatcher {
    pub slots: VecDeque<BatchSlot>,
    pub max_slots: usize,
}

impl ContinuousBatcher {
    /// Create a new batcher with the given maximum slot count.
    ///
    /// ## Parameters
    /// - `max_slots`: Maximum number of concurrent requests
    ///
    /// ## Returns
    /// A new `ContinuousBatcher`.
    pub fn new(max_slots: usize) -> Self {
        Self {
            slots: VecDeque::with_capacity(max_slots),
            max_slots,
        }
    }

    /// Try to admit a new request into the batch.
    ///
    /// ## Parameters
    /// - `job`: The job ID to admit
    ///
    /// ## Returns
    /// `true` if admitted, `false` if the batch is full.
    pub fn admit(&mut self, job: JobId) -> bool {
        if self.slots.len() >= self.max_slots {
            return false;
        }
        self.slots.push_back(BatchSlot {
            job,
            tokens_generated: 0,
        });
        true
    }

    /// Remove a completed request from the batch.
    ///
    /// ## Parameters
    /// - `job`: The job ID to release
    pub fn release(&mut self, job: JobId) {
        self.slots.retain(|s| s.job != job);
    }

    /// Returns the next job to process.
    ///
    /// ## Returns
    /// `Option<JobId>` — the next job, or None if empty.
    pub fn next(&mut self) -> Option<JobId> {
        self.slots.front().map(|s| s.job)
    }

    /// Returns true if the batch is at capacity.
    pub fn is_full(&self) -> bool {
        self.slots.len() >= self.max_slots
    }

    /// Returns the number of in-flight requests.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns true if there are no in-flight requests.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admit_and_release() {
        let mut b = ContinuousBatcher::new(2);
        let j1 = JobId::new();
        let j2 = JobId::new();

        assert!(b.admit(j1));
        assert!(b.admit(j2));
        assert!(!b.admit(JobId::new())); // full
        assert_eq!(b.len(), 2);

        b.release(j1);
        assert_eq!(b.len(), 1);
        assert!(b.admit(JobId::new())); // has room now
    }

    #[test]
    fn next_returns_front() {
        let mut b = ContinuousBatcher::new(5);
        let j1 = JobId::new();
        b.admit(j1);
        assert_eq!(b.next(), Some(j1));
    }

    #[test]
    fn empty_batcher() {
        let b = ContinuousBatcher::new(5);
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.len(), 0);
    }
}
