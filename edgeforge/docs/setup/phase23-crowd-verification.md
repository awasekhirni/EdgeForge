# EdgeForge — Phase 23: Crowd Tier 2/3 Verification Setup Guide

**Version:** 23.0
**Date:** 2026-11-01

This guide covers the Crowd Tier 2/3 verification pipeline: PoQ sampling,
quorum calculation, verdict aggregation, and the `VerificationOutcome`
production.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Tier 2 Verification](#quick-start-tier-2-verification)
3. [PoQ Sampling](#poq-sampling)
4. [Quorum Calculation](#quorum-calculation)
5. [Verdict Aggregation](#verdict-aggregation)
6. [API Reference](#api-reference)
7. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                  Crowd Tier 2/3 Verification                 │
│                                                              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐     │
│  │ route_expert │──▶│ PoQSampler   │──▶│ Verifiers    │     │
│  │ _fetch()     │   │ (sample_rate)│   │ (N peers)    │     │
│  └──────────────┘   └──────────────┘   └──────┬───────┘     │
│                                                │             │
│                                                ▼             │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐     │
│  │ aggregate    │◀──│ QuorumCalc   │◀──│ record       │     │
│  │ _verdicts()  │   │ (majority)   │   │ _verdict()   │     │
│  └──────┬───────┘   └──────────────┘   └──────────────┘     │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐     │
│  │ Verification │──▶│ Reputation  │──▶│ Incentive    │     │
│  │ Outcome      │   │ Graph       │   │ Ledger       │     │
│  └──────────────┘   └──────────────┘   └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### Three Tiers

| Tier | Name | Verification | Peers | Redundancy |
|---|---|---|---|---|
| 1 | Trusted | None (mTLS) | ≤16 | 1 |
| 2 | Cooperative | PoQ sampling | ≤100 | 2 |
| 3 | Open | Optimistic + eventual | Unbounded | 3 |

### Verification Pipeline

1. **`route_expert_fetch(layer, expert, tier)`** — selects the fetch peer
   + PoQ-sampled verifiers
2. **Peers verify the result** — each verifier produces a `PeerVerdict`
3. **`record_verdict(event)`** — stores each verdict + updates reputation +
   posts to the incentive ledger
4. **`aggregate_verdicts(job)`** — checks quorum + aggregates into a single
   `VerificationOutcome`

---

## Quick Start: Tier 2 Verification

```rust
use edgeforge_crowd::*;
use edgeforge_core::{ControlEvent, ControlEventKind, CrowdTier, ExpertId, LayerIdx, NodeId, VerificationOutcome};

// 1. Set up a Tier 2 coordinator with PoQ sampling.
let mode = ExecutionMode::DistributedCooperative {
    verification: VerificationPolicy {
        deadline_ms: 50,
        sampler: VerificationSampler::ProofOfQuality { sample_rate: 0.5 },
        redundancy: 2,
    },
    redundancy: RedundancyPolicy::default(),
};
let coordinator = DefaultCrowdCoordinator::new(mode);

// 2. Admit peers.
let fetch_peer = NodeId(ulid::Ulid::new());
let verifier1 = NodeId(ulid::Ulid::new());
let verifier2 = NodeId(ulid::Ulid::new());
coordinator.admit_peer(fetch_peer, CrowdTier::Cooperative, PeerCapability {
    hosted_experts: vec![(LayerIdx(0), ExpertId(1))],
    ..Default::default()
}).await?;
coordinator.admit_peer(verifier1, CrowdTier::Cooperative, PeerCapability::default()).await?;
coordinator.admit_peer(verifier2, CrowdTier::Cooperative, PeerCapability::default()).await?;

// 3. Route the expert fetch — returns fetch peer + PoQ verifiers.
let selected = coordinator.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Cooperative).await?;
// selected = [fetch_peer, verifier1, verifier2]

// 4. Simulate verifiers submitting verdicts.
let job = JobId(ulid::Ulid::new());
for peer in selected.iter().skip(1) { // skip fetch peer
    let event = ControlEvent {
        timestamp_ms: 0,
        kind: ControlEventKind::VerificationVerdict {
            job, peer: *peer, tier: CrowdTier::Cooperative,
            outcome: VerificationOutcome::Verified, latency_ms: 10,
        },
    };
    coordinator.record_verdict(event).await?;
}

// 5. Aggregate — produces the final VerificationOutcome.
let result = coordinator.aggregate_verdicts(&job).await?;
assert_eq!(result.outcome, VerificationOutcome::Verified);
```

---

## PoQ Sampling

`PoQSampler::sample(candidates, sample_rate)` selects `ceil(sample_rate * len)`
peers from the candidate list, sorted by quality score (highest first).

### Algorithm

1. Sort candidates by quality score (descending)
2. Take the top `ceil(sample_rate * len)` peers
3. Return the selected `NodeId`s

### Example

```rust
let candidates = vec![
    (peer1, 0.3),  // quality score 0.3
    (peer2, 0.9),  // quality score 0.9
    (peer3, 0.6),  // quality score 0.6
];
// sample_rate = 0.34 → ceil(0.34 * 3) = 2 → select top 2
let selected = PoQSampler::sample(&candidates, 0.34);
// selected = [peer2, peer3] (sorted by quality)
```

---

## Quorum Calculation

`QuorumCalculator` determines if enough verifiers have responded:

- **Threshold**: `⌈total_expected / 2⌉ + 1` (strict majority)
- **`quorum_reached(verdicts_received, total_expected)`**: `true` if
  `verdicts_received >= threshold`

### Example

```rust
// 5 verifiers expected → threshold = (5+1)/2 + 1 = 4
assert_eq!(QuorumCalculator::threshold(5), 4);
assert!(!QuorumCalculator::quorum_reached(3, 5)); // 3 < 4
assert!(QuorumCalculator::quorum_reached(4, 5));  // 4 >= 4
```

---

## Verdict Aggregation

`VerdictAggregator::aggregate(verdicts, quorum_reached, deadline_elapsed)`:

| Condition | Outcome |
|---|---|
| Deadline elapsed, no quorum | `Pending` |
| No quorum, deadline not elapsed | `Pending` |
| Quorum reached, all `Verified` | `Verified` |
| Quorum reached, majority `Corrected` | `Corrected` |
| Quorum reached, mixed (no majority) | `Pending` |

### `VerificationOutcome` Values

| Outcome | Meaning |
|---|---|
| `NotRequired` | Tier 1 — no verification needed |
| `Verified` | Tier 2/3 — quorum of verifiers agree the result is correct |
| `Pending` | Tier 2/3 — verification not yet complete (no quorum or mixed verdicts) |
| `Corrected` | Tier 2/3 — majority of verifiers say the result was wrong |

---

## API Reference

### `CrowdCoordinator` trait

| Method | Returns | Purpose |
|---|---|---|
| `admit_peer(peer, tier, cap)` | `Result<(), CrowdError>` | Admit a new peer |
| `route_expert_fetch(layer, expert, tier)` | `Result<Vec<NodeId>, CrowdError>` | Select fetch peer + verifiers |
| `record_verdict(event)` | `Result<(), CrowdError>` | Store a peer's verdict + update reputation/ledger |
| `aggregate_verdicts(job)` | `Result<VerificationResult, CrowdError>` | Aggregate pending verdicts |
| `evict_peer(peer)` | `Result<(), CrowdError>` | Evict a peer |

### `PoQSampler`

| Method | Returns | Purpose |
|---|---|---|
| `sample(candidates, sample_rate)` | `Vec<NodeId>` | Select top-k peers by quality |

### `QuorumCalculator`

| Method | Returns | Purpose |
|---|---|---|
| `threshold(total_expected)` | `usize` | Majority threshold |
| `quorum_reached(received, expected)` | `bool` | Check if quorum is reached |

### `VerdictAggregator`

| Method | Returns | Purpose |
|---|---|---|
| `aggregate(verdicts, quorum, deadline)` | `VerificationResult` | Aggregate into outcome |

---

## Troubleshooting

### "route_expert_fetch returns empty Vec"

This was a **Phase 22 bug**. It is fixed: `route_expert_fetch` now selects
real peers based on tier, reputation, and hosted experts. Update to the
latest build.

### "record_verdict does nothing"

This was a **Phase 22 bug**. It is fixed: `record_verdict` now stores the
verdict, updates the peer's reputation (success for Verified, failure for
Corrected), and posts to the incentive ledger.

### "VerificationOutcome is never produced"

This was a **Phase 22 bug**. It is fixed: `aggregate_verdicts` now produces
real `VerificationOutcome` values (Verified, Pending, Corrected) based on
quorum + majority aggregation.

### "NoPeersAvailable error"

No peers have been admitted at the requested tier. Call `admit_peer` first:

```rust
coordinator.admit_peer(peer, CrowdTier::Cooperative, capability).await?;
```

### "aggregate_verdicts returns NotRequired"

No verdicts have been recorded for the job. Either:
- The job is Tier 1 (no verification needed)
- `record_verdict` was never called
- The job was already finalized (Verified/Corrected removes the pending entry)
