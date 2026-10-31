// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod engine;
pub mod mock_engine;
pub mod router_hook;
pub mod speculation;
pub use engine::{EngineError, GenParams, GenStats, InferenceEngine, LoadedModel};
pub use mock_engine::MockInferenceEngine;
pub use router_hook::{
    ExpertResolution, GraphAwareRouterHook, RamOnlyRouterHook, RouterHook, SsdOnlyRouterHook,
};
pub use speculation::{
    detect_workload, permit_surrogate, strategy_for_workload, SpeculationEngine,
    SpeculationStrategy, WorkloadType,
};
