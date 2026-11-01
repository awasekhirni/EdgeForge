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
pub struct Scheduler { interactive: VecDeque<JobId>, batch: VecDeque<JobId> }
impl Scheduler {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self { Self { interactive: VecDeque::new(), batch: VecDeque::new() } }
    /// Enqueues a new item.
    pub fn enqueue(&mut self, job: JobId, priority: crate::JobPriority) { match priority { crate::JobPriority::Interactive => self.interactive.push_back(job), crate::JobPriority::Batch => self.batch.push_back(job) } }
    /// Picks the next item.
    pub fn pick(&mut self) -> Option<JobId> { self.interactive.pop_front().or_else(|| self.batch.pop_front()) }
}
impl Default for Scheduler { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobPriority;
    use ulid::Ulid;

    fn job() -> JobId { JobId(Ulid::new()) }

    #[test]
    fn test_empty_scheduler_returns_none() {
        let mut s = Scheduler::new();
        assert!(s.pick().is_none());
    }

    #[test]
    fn test_enqueue_and_pick_batch() {
        let mut s = Scheduler::new();
        let j = job();
        s.enqueue(j, JobPriority::Batch);
        assert_eq!(s.pick(), Some(j));
    }

    #[test]
    fn test_enqueue_and_pick_interactive() {
        let mut s = Scheduler::new();
        let j = job();
        s.enqueue(j, JobPriority::Interactive);
        assert_eq!(s.pick(), Some(j));
    }

    #[test]
    fn test_interactive_preempts_batch() {
        let mut s = Scheduler::new();
        let b = job();
        let i = job();
        s.enqueue(b, JobPriority::Batch);
        s.enqueue(i, JobPriority::Interactive);
        assert_eq!(s.pick(), Some(i));
        assert_eq!(s.pick(), Some(b));
    }

    #[test]
    fn test_fifo_within_batch() {
        let mut s = Scheduler::new();
        let j1 = job();
        let j2 = job();
        let j3 = job();
        s.enqueue(j1, JobPriority::Batch);
        s.enqueue(j2, JobPriority::Batch);
        s.enqueue(j3, JobPriority::Batch);
        assert_eq!(s.pick(), Some(j1));
        assert_eq!(s.pick(), Some(j2));
        assert_eq!(s.pick(), Some(j3));
    }

    #[test]
    fn test_fifo_within_interactive() {
        let mut s = Scheduler::new();
        let j1 = job();
        let j2 = job();
        s.enqueue(j1, JobPriority::Interactive);
        s.enqueue(j2, JobPriority::Interactive);
        assert_eq!(s.pick(), Some(j1));
        assert_eq!(s.pick(), Some(j2));
    }

    #[test]
    fn test_mixed_priority_ordering() {
        let mut s = Scheduler::new();
        let b1 = job();
        let i1 = job();
        let b2 = job();
        let i2 = job();
        s.enqueue(b1, JobPriority::Batch);
        s.enqueue(i1, JobPriority::Interactive);
        s.enqueue(b2, JobPriority::Batch);
        s.enqueue(i2, JobPriority::Interactive);
        // Interactive first (FIFO), then batch (FIFO)
        assert_eq!(s.pick(), Some(i1));
        assert_eq!(s.pick(), Some(i2));
        assert_eq!(s.pick(), Some(b1));
        assert_eq!(s.pick(), Some(b2));
    }

    #[test]
    fn test_pick_from_empty_after_drain() {
        let mut s = Scheduler::new();
        s.enqueue(job(), JobPriority::Batch);
        s.pick();
        assert!(s.pick().is_none());
    }
}
