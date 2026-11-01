# ADR-012: OpenTelemetry OTLP Exporter (Opt-In)

**Status:** Approved
**Date:** 2026-10-31

## Decision

`tracing` is always initialised for structured logs. The OTLP exporter is behind the `telemetry-otlp` feature flag and is **off by default** per ADR-008.

## Safety (R-14)

Span attributes must never include prompt content, message text, or user data. The `FORBIDDEN_ATTRIBUTE_SUBSTRINGS` list (`prompt`, `content`, `text`, `message`, `secret`, `api_key`, `password`, `private`) is enforced by the `safe_attributes()` helper. CI asserts no span attribute key matches the denylist.
