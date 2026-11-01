# ADR-007: OpenAI-Compatible REST API

**Status:** Approved
**Date:** 2026-10-31

## Decision

The local API server is OpenAI-compatible, listening on `127.0.0.1:11434`. Endpoints:

- `POST /v1/chat/completions` — chat (SSE streaming when `stream: true`)
- `POST /v1/completions` — text completion
- `POST /v1/embeddings` — RAG embeddings
- `GET /v1/models` — list models
- `GET /health` — liveness

## SSE Headers

- `X-EdgeForge-Verification: verified|pending|corrected` — async verification status (ADR-018)

## Drop-In Replacement

Any OpenAI SDK works by setting `base_url=http://127.0.0.1:11434/v1` and `api_key="not-needed"`.
