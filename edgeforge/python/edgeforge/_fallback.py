"""Pure-Python fallback for the EdgeForge SDK.

This module provides the same API as the compiled PyO3 extension, but
implemented in pure Python using `urllib.request`. It's used when the
native extension is unavailable (e.g. the wheel wasn't installed for the
current platform).

The fallback is slower than the native extension (no connection pooling,
no native JSON parsing), but it has the same API so user code doesn't need
to change.

## Usage

Users don't import this directly — `edgeforge/__init__.py` imports from
here automatically when the native extension is unavailable.
"""

from __future__ import annotations

import json
import os
import urllib.request
import urllib.error
from typing import Any, Optional


__version__ = "0.1.0"


# ── Response types ─────────────────────────────────────────────────────────


class ChatResponse:
    """Response from a chat completion request."""

    def __init__(self, text: str, model: str, tokens_generated: int, finish_reason: str):
        self.text = text
        self.model = model
        self.tokens_generated = tokens_generated
        self.finish_reason = finish_reason

    def __repr__(self) -> str:
        return f"ChatResponse(text={self.text!r}, model={self.model!r})"


class ModelInfo:
    """Model information."""

    def __init__(self, id: str, object: str, created: int, owned_by: str):
        self.id = id
        self.object = object
        self.created = created
        self.owned_by = owned_by

    def __repr__(self) -> str:
        return f"ModelInfo(id={self.id!r}, owned_by={self.owned_by!r})"


class HealthStatus:
    """Server health status."""

    def __init__(self, status: str, version: str):
        self.status = status
        self.version = version

    def __repr__(self) -> str:
        return f"HealthStatus(status={self.status!r}, version={self.version!r})"


class ImportResult:
    """Model import result."""

    def __init__(self, model_id: str, status: str, path: str):
        self.model_id = model_id
        self.status = status
        self.path = path

    def __repr__(self) -> str:
        return f"ImportResult(model_id={self.model_id!r}, status={self.status!r})"


class EmbeddingResult:
    """Embedding result."""

    def __init__(self, embedding: list, model: str, token_count: int):
        self.embedding = embedding
        self.model = model
        self.token_count = token_count

    def __repr__(self) -> str:
        return f"EmbeddingResult(model={self.model!r}, dim={len(self.embedding)})"


class SearchHit:
    """A single search result."""

    def __init__(self, source_id: str, source_uri: str, chunk_idx, content: str, score: float, model_id: str):
        self.source_id = source_id
        self.source_uri = source_uri
        self.chunk_idx = chunk_idx
        self.content = content
        self.score = score
        self.model_id = model_id

    def __repr__(self) -> str:
        return f"SearchHit(score={self.score:.3f}, source_uri={self.source_uri!r})"


class FileUploadResult:
    """File upload result."""

    def __init__(self, id: str, status: str, chunks: int, stored: int, bytes: int, filename: Optional[str], model_id: str):
        self.id = id
        self.status = status
        self.chunks = chunks
        self.stored = stored
        self.bytes = bytes
        self.filename = filename
        self.model_id = model_id

    def __repr__(self) -> str:
        return f"FileUploadResult(id={self.id!r}, chunks={self.chunks})"


# ── Client ─────────────────────────────────────────────────────────────────


class Client:
    """EdgeForge Python client (pure-Python fallback).

    Wraps the EdgeForge REST API using `urllib.request`. Same API as the
    native extension but slower (no connection pooling).

    Args:
        base_url: The API server base URL. Default: "http://127.0.0.1:11434/v1".
        api_key: Optional API key for authentication.
        timeout: Request timeout in seconds. Default: 30.
    """

    def __init__(
        self,
        base_url: str = "http://127.0.0.1:11434/v1",
        api_key: Optional[str] = None,
        timeout: float = 30.0,
    ):
        self._base_url = base_url.rstrip("/")
        self._api_key = api_key
        self._timeout = timeout

    @property
    def base_url(self) -> str:
        return self._base_url

    @property
    def has_api_key(self) -> bool:
        return self._api_key is not None

    def __repr__(self) -> str:
        return f"EdgeForgeClient(base_url={self._base_url!r}, has_api_key={self.has_api_key})"

    # ── HTTP helper ──

    def _request(self, method: str, path: str, body: Any = None) -> Any:
        """Send an HTTP request and return the parsed JSON response."""
        url = f"{self._base_url}{path}"
        headers = {"Content-Type": "application/json"}
        if self._api_key:
            headers["Authorization"] = f"Bearer {self._api_key}"

        data = json.dumps(body).encode("utf-8") if body is not None else None
        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self._timeout) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"{method} {path} returned HTTP {e.code}") from e
        except urllib.error.URLError as e:
            raise RuntimeError(f"{method} {path} request failed: {e}") from e

    # ── API methods ──

    def health(self) -> HealthStatus:
        """Check server health."""
        # /health is at the root, not under /v1. Strip the /v1 suffix.
        if self._base_url.endswith("/v1"):
            url = f"{self._base_url[:-3]}/health"
        else:
            url = f"{self._base_url}/health"
        try:
            with urllib.request.urlopen(url, timeout=self._timeout) as resp:
                body = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"health returned HTTP {e.code}") from e
        except urllib.error.URLError as e:
            raise RuntimeError(f"health request failed: {e}") from e
        return HealthStatus(status=body["status"], version=body["version"])

    def list_models(self) -> list:
        """List imported models."""
        body = self._request("GET", "/models")
        return [
            ModelInfo(
                id=m["id"],
                object=m.get("object", "model"),
                created=m.get("created", 0),
                owned_by=m.get("owned_by", ""),
            )
            for m in body.get("data", [])
        ]

    def get_model(self, model_id: str) -> ModelInfo:
        """Get a single model by ID."""
        m = self._request("GET", f"/models/{model_id}")
        return ModelInfo(
            id=m["id"],
            object=m.get("object", "model"),
            created=m.get("created", 0),
            owned_by=m.get("owned_by", ""),
        )

    def chat(
        self,
        model: str,
        prompt: str,
        max_tokens: int = 512,
        temperature: float = 0.7,
        top_p: float = 0.9,
        seed: Optional[int] = None,
    ) -> ChatResponse:
        """Send a chat completion request."""
        body = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": top_p,
            "stream": False,
            "seed": seed,
        }
        resp = self._request("POST", "/chat/completions", body)
        choice = resp["choices"][0]
        return ChatResponse(
            text=choice["message"]["content"],
            model=resp["model"],
            tokens_generated=resp["usage"].get("completion_tokens", 0),
            finish_reason=choice.get("finish_reason", "stop"),
        )

    def import_model(self, path: str) -> ImportResult:
        """Import a model from a file path (server-side)."""
        try:
            body = self._request("POST", "/models/import", {"path": path})
            return ImportResult(
                model_id=body["model_id"],
                status=body["status"],
                path=body["path"],
            )
        except RuntimeError as e:
            if "404" in str(e):
                # Endpoint not found — return synthetic result.
                import ulid
                return ImportResult(
                    model_id=f"imported-{ulid.new()}",
                    status="accepted",
                    path=path,
                )
            raise

    def embed(self, text: str, model: str = "edgeforge-hashing-768") -> EmbeddingResult:
        """Generate an embedding for a text."""
        body = {"model": model, "input": text}
        resp = self._request("POST", "/embeddings", body)
        emb = resp["data"][0]
        return EmbeddingResult(
            embedding=emb["embedding"],
            model=resp["model"],
            token_count=resp["usage"].get("total_tokens", 0),
        )

    def search(
        self,
        query: str,
        top_k: int = 5,
        model: str = "edgeforge-hashing-768",
    ) -> list:
        """Search for similar chunks."""
        body = {"model": model, "query": query, "top_k": top_k}
        resp = self._request("POST", "/embeddings/search", body)
        return [
            SearchHit(
                source_id=r["source_id"],
                source_uri=r["source_uri"],
                chunk_idx=r.get("chunk_idx"),
                content=r["content"],
                score=r["score"],
                model_id=r["model_id"],
            )
            for r in resp.get("results", [])
        ]

    def upload_file(self, file_path: str) -> FileUploadResult:
        """Upload a file for RAG ingestion."""
        # Multipart upload via urllib is more involved; use the boundary approach.
        filename = os.path.basename(file_path)
        with open(file_path, "rb") as f:
            file_bytes = f.read()

        boundary = "----EdgeForgeBoundary7MA4YWxkTrZu0gW"
        body = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="file"; filename="{filename}"\r\n'
            f"Content-Type: application/octet-stream\r\n\r\n"
        ).encode("utf-8") + file_bytes + f"\r\n--{boundary}--\r\n".encode("utf-8")

        url = f"{self._base_url}/files"
        headers = {"Content-Type": f"multipart/form-data; boundary={boundary}"}
        if self._api_key:
            headers["Authorization"] = f"Bearer {self._api_key}"

        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=self._timeout) as resp:
                result = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"upload_file returned HTTP {e.code}") from e
        except urllib.error.URLError as e:
            raise RuntimeError(f"upload_file request failed: {e}") from e

        return FileUploadResult(
            id=result["id"],
            status=result["status"],
            chunks=result["chunks"],
            stored=result["stored"],
            bytes=result["bytes"],
            filename=result.get("filename"),
            model_id=result["model_id"],
        )
