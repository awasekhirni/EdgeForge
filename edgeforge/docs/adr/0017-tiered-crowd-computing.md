# ADR-017: Tiered Crowd Computing with Optimistic Verification

**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision

Three-tier distribution model replacing the binary trusted/untrusted policy:

| Tier | Peers | Trust | Verification | Incentive | UI Label |
|---|---|---|---|---|---|
| 1: Trusted Mesh | ≤16, paired | mTLS | Activation checksums | None (reciprocal) | "Trusted Pool" |
| 2: Cooperative Pool | ≤100, org identity | Org-level (SSO) | Proof of Quality (PoQ) | Credit-based | "Cooperative Pool" |
| 3: Open Crowd | Unbounded, open | Economic stake/PoW | Optimistic + eventual | Token-based | "Experimental: Open Crowd" |

## MoE Advantage

- RTX 3090 volunteer hosts 2-3 Q4_K_M experts (~1.5 GB each)
- Only ~25% of parameters activate per token — no dormant compute cost
- Router dispatches to cheapest available expert provider

## Honesty Contract (verbatim)

> **Distributed mode increases capacity, not speed.**
> Verification adds latency. Only enable distributed mode for models that cannot fit locally. Tier 3 (Open Crowd) is experimental and may produce incorrect outputs that are corrected retroactively.

## Invariants

1. Tier 3 is opt-in only; no code path may auto-enable it.
2. Tier 3 peers never see raw prompts — only decontextualized activations (R-20).
3. Verification never blocks the token stream beyond 50 ms (ADR-018, R-21).
4. Tier 3 is labeled "Experimental" in the UI with a persistent watermark.
