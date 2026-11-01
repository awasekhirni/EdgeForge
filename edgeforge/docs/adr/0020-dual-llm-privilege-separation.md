# ADR-020: Dual-LLM Privilege Separation
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v3.0

## Decision
Privileged planner LLM (sees prompt, no tools) + unprivileged worker LLM (tools, no prompt). Planner produces ActionPlan; worker executes with spotlighted context only. Post-decode guardrail validates action against plan.

## Invariants
1. Planner has no tool access.
2. Worker never sees the original prompt.
3. Every action is validated against the plan.
