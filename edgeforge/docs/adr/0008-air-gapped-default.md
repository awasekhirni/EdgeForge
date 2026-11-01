# ADR-008: Air-Gapped Default

**Status:** Approved
**Date:** 2026-10-31

## Decision

**Air-gapped is the default.** All networking — mesh, crowd, telemetry — is opt-in. No outbound network calls unless the user explicitly enables them.

## Config

```toml
[network]
mode = "air_gapped"  # default
telemetry = false    # default
```

## Consequences

- The `ControlEvent` bus never carries prompt content (R-14, R-20).
- OTLP telemetry is behind a feature flag, off by default (ADR-012).
- Tier 3 (Open Crowd) is opt-in and labeled "Experimental".
