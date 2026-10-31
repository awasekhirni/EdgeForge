// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::JobId;
use std::collections::VecDeque;
pub struct Scheduler {
    interactive: VecDeque<JobId>,
    batch: VecDeque<JobId>,
}
impl Scheduler {
    pub fn new() -> Self {
        Self {
            interactive: VecDeque::new(),
            batch: VecDeque::new(),
        }
    }
    pub fn enqueue(&mut self, job: JobId, priority: crate::JobPriority) {
        match priority {
            crate::JobPriority::Interactive => self.interactive.push_back(job),
            crate::JobPriority::Batch => self.batch.push_back(job),
        }
    }
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
