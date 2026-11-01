//! # Priority Lanes: Interactive vs Batch (F-SCHED-04)
//!
//! ## Purpose
//! Two-slot scheduler. Interactive preempts batch at the layer boundary
//! to keep chat responsive while batch jobs run in the background.
//!
//! ## What This File Does
//! - `Priority` — Interactive or Batch
//! - `PriorityScheduler` — two-slot scheduler (one interactive, one batch)
//! - `admit(job, priority)` — admit a job into the appropriate lane
//! - `release(job)` — release a job from its lane
//! - `next()` — which job should run next (interactive always preempts)
//!
//! ## Dependencies
//! - `edgeforge_core::JobId`
//!
//! ## Output
//! `Option<JobId>` from next() — the job to run, or None.

use edgeforge_core::JobId;

/// Priority lane for job scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Interactive,
    Batch,
}

/// Two-slot priority scheduler.
pub struct PriorityScheduler {
    pub interactive: Option<JobId>,
    pub batch: Option<JobId>,
}

impl PriorityScheduler {
    /// Create a new scheduler with empty slots.
    pub fn new() -> Self {
        Self {
            interactive: None,
            batch: None,
        }
    }

    /// Admit a job into the given priority lane.
    ///
    /// ## Parameters
    /// - `job`: The job to admit
    /// - `priority`: Interactive or Batch
    ///
    /// ## Returns
    /// `true` if admitted, `false` if the lane is occupied.
    pub fn admit(&mut self, job: JobId, priority: Priority) -> bool {
        match priority {
            Priority::Interactive => {
                if self.interactive.is_some() {
                    return false;
                }
                self.interactive = Some(job);
                true
            }
            Priority::Batch => {
                if self.batch.is_some() {
                    return false;
                }
                self.batch = Some(job);
                true
            }
        }
    }

    /// Release a job from its lane.
    pub fn release(&mut self, job: JobId) {
        if self.interactive == Some(job) {
            self.interactive = None;
        }
        if self.batch == Some(job) {
            self.batch = None;
        }
    }

    /// Which job should run next?
    ///
    /// Interactive always preempts batch.
    pub fn next(&self) -> Option<JobId> {
        self.interactive.or(self.batch)
    }
}

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_preempts_batch() {
        let mut s = PriorityScheduler::new();
        let bi = JobId::new();
        let bb = JobId::new();
        s.admit(bb, Priority::Batch);
        s.admit(bi, Priority::Interactive);
        assert_eq!(s.next(), Some(bi));
    }

    #[test]
    fn batch_runs_when_interactive_empty() {
        let mut s = PriorityScheduler::new();
        let bb = JobId::new();
        s.admit(bb, Priority::Batch);
        assert_eq!(s.next(), Some(bb));
    }

    #[test]
    fn release_frees_slot() {
        let mut s = PriorityScheduler::new();
        let j = JobId::new();
        s.admit(j, Priority::Interactive);
        s.release(j);
        assert!(s.next().is_none());
    }
}
