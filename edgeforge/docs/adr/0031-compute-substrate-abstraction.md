# ADR-031: Compute Substrate Abstraction (Quantum/Neuromorphic Ready)
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
ComputeSubstrate trait in edgeforge-core-legacy. Today: ClassicalCpuGpu. Future: QuantumAnnealing, Neuromorphic, NeuralInterface. ControlEventKind is #[non_exhaustive] for forward compatibility (NeuralSpikeEvent).
