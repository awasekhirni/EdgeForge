# ADR-019: ControlEvent Schema as Unified Event Bus
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision
Typed ControlEvent enum with 8 event kinds (RouterDecision, IoCompletion, PeerHeartbeat, PeerLost, ThermalSample, SpeculationVerdict, TokenGenerated, VerificationVerdict). Events NEVER carry prompt content (R-14, R-20). Lossy under load (bounded channel, drops oldest).
