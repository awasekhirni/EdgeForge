# ADR-023: Quarantine Reader LLM
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v3.0

## Decision
Separate isolated 0.5B LLM with no tools, no prompt access, fixed non-overridable system prompt. Reads raw external content, produces ≤200-word sanitized summary. Confidence gate drops summaries <0.5. Main model never sees raw external content.

## Reference
Hines et al. (2024) "Defending Against Indirect Prompt Injection Attacks With Spotlighting"
