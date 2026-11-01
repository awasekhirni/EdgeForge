# ADR-022: Intent Guardrails
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v3.0

## Decision
Two-stage: (1) pre-flight classify intent against denylist (DataExfiltration, ArbitraryShell, SystemModification, PromptRevelation); (2) post-decode validate action ∈ permitted_tools + step index in range. Pluggable classifier (default: regex).
