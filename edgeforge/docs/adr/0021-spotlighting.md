# ADR-021: Spotlighting
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v3.0

## Decision
Three techniques on all untrusted external content: data marking ([UNTRUSTED_BEGIN]...[END]), data encapsulation (base64), data trimming (strip injection patterns). Default: marking + trimming on; encapsulation opt-in.
