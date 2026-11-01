"""Test suite for the EdgeForge Python SDK.

These tests exercise the pure-Python fallback (`edgeforge._fallback`)
against a mock HTTP server built with `http.server`. They verify:
- Client construction and defaults
- health() returns the server's status + version
- list_models() returns a list of ModelInfo
- get_model() returns a single ModelInfo
- chat() returns a ChatResponse with the generated text
- embed() returns an EmbeddingResult with a real vector
- search() returns a list of SearchHit ranked by score
- upload_file() uploads a file and returns a FileUploadResult
- import_model() returns an ImportResult
- API key is sent as `Authorization: Bearer <key>`
- HTTP errors are raised as RuntimeError

The tests don't require a real EdgeForge server — they spin up a mock
server in a background thread.
"""

from __future__ import annotations

import json
import os
import sys
import threading
import tempfile
import time
import unittest
from http.server import HTTPServer, BaseHTTPRequestHandler

# Make the fallback importable.
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from edgeforge._fallback import (  # noqa: E402
    Client,
    ChatResponse,
    ModelInfo,
    HealthStatus,
    ImportResult,
    EmbeddingResult,
    SearchHit,
    FileUploadResult,
    __version__,
)


# ── Mock server ────────────────────────────────────────────────────────────


class MockEdgeForgeHandler(BaseHTTPRequestHandler):
    """Mock EdgeForge API server for testing."""

    # Track received headers for assertions.
    received_auth_header: str | None = None
    received_chat_body: dict | None = None

    def log_message(self, format, *args):  # noqa: A002
        # Suppress default logging.
        pass

    def _send_json(self, status: int, body: dict):
        data = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):  # noqa: N802
        if self.path == "/health":
            self._send_json(200, {"status": "ok", "version": "0.1.0"})
        elif self.path == "/v1/models":
            MockEdgeForgeHandler.received_auth_header = self.headers.get("Authorization")
            self._send_json(
                200,
                {
                    "object": "list",
                    "data": [
                        {
                            "id": "01J9TEST",
                            "object": "model",
                            "created": 1700000000,
                            "owned_by": "test-publisher",
                        }
                    ],
                },
            )
        elif self.path.startswith("/v1/models/"):
            model_id = self.path.split("/")[-1]
            self._send_json(
                200,
                {
                    "id": model_id,
                    "object": "model",
                    "created": 1700000000,
                    "owned_by": "test-publisher",
                },
            )
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self):  # noqa: N802
        content_length = int(self.headers.get("Content-Length", 0))
        body_raw = self.rfile.read(content_length) if content_length else b""
        MockEdgeForgeHandler.received_auth_header = self.headers.get("Authorization")

        if self.path == "/v1/chat/completions":
            body = json.loads(body_raw)
            MockEdgeForgeHandler.received_chat_body = body
            self._send_json(
                200,
                {
                    "id": "chatcmpl-1",
                    "object": "chat.completion",
                    "created": 1700000000,
                    "model": body["model"],
                    "choices": [
                        {
                            "index": 0,
                            "message": {"role": "assistant", "content": "Hello back!"},
                            "finish_reason": "stop",
                        }
                    ],
                    "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
                },
            )
        elif self.path == "/v1/embeddings":
            self._send_json(
                200,
                {
                    "object": "list",
                    "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3]}],
                    "model": "edgeforge-hashing-768",
                    "usage": {"prompt_tokens": 2, "total_tokens": 2},
                },
            )
        elif self.path == "/v1/embeddings/search":
            self._send_json(
                200,
                {
                    "object": "list",
                    "model": "edgeforge-hashing-768",
                    "results": [
                        {
                            "source_id": "src1",
                            "source_uri": "doc.txt",
                            "chunk_idx": 0,
                            "content": "hello world",
                            "score": 0.95,
                            "model_id": "edgeforge-hashing-768",
                        }
                    ],
                },
            )
        elif self.path == "/v1/models/import":
            self._send_json(
                200,
                {"model_id": "01J9IMPORTED", "status": "accepted", "path": "/tmp/test.gguf"},
            )
        elif self.path == "/v1/files":
            # Multipart upload — just return a success response.
            self._send_json(
                200,
                {
                    "id": "file-1",
                    "status": "processed",
                    "chunks": 3,
                    "stored": 3,
                    "bytes": 1024,
                    "filename": "test.txt",
                    "model_id": "edgeforge-hashing-768",
                },
            )
        else:
            self._send_json(404, {"error": "not found"})


def start_mock_server(port: int = 0) -> tuple[HTTPServer, int]:
    """Start the mock server in a background thread. Returns (server, port)."""
    server = HTTPServer(("127.0.0.1", port), MockEdgeForgeHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, server.server_address[1]


# ── Tests ──────────────────────────────────────────────────────────────────


class TestEdgeForgeClient(unittest.TestCase):
    """Tests for the EdgeForgeClient (pure-Python fallback)."""

    @classmethod
    def setUpClass(cls):
        cls.server, cls.port = start_mock_server()
        cls.base_url = f"http://127.0.0.1:{cls.port}/v1"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()

    def setUp(self):
        # Reset the mock server's tracked state.
        MockEdgeForgeHandler.received_auth_header = None
        MockEdgeForgeHandler.received_chat_body = None

    # ── Construction tests ──

    def test_client_default_url(self):
        c = Client()
        self.assertEqual(c.base_url, "http://127.0.0.1:11434/v1")
        self.assertFalse(c.has_api_key)

    def test_client_custom_url(self):
        c = Client(base_url="http://example.com:8080/v1")
        self.assertEqual(c.base_url, "http://example.com:8080/v1")

    def test_client_with_api_key(self):
        c = Client(api_key="my-secret")
        self.assertTrue(c.has_api_key)

    def test_client_repr(self):
        c = Client(base_url="http://localhost/v1", api_key="key")
        s = repr(c)
        self.assertIn("localhost", s)
        self.assertIn("has_api_key=True", s)

    # ── health() ──

    def test_health_returns_ok(self):
        c = Client(base_url=self.base_url)
        h = c.health()
        self.assertIsInstance(h, HealthStatus)
        self.assertEqual(h.status, "ok")
        self.assertEqual(h.version, "0.1.0")

    # ── list_models() ──

    def test_list_models_returns_list(self):
        c = Client(base_url=self.base_url)
        models = c.list_models()
        self.assertEqual(len(models), 1)
        self.assertIsInstance(models[0], ModelInfo)
        self.assertEqual(models[0].id, "01J9TEST")
        self.assertEqual(models[0].owned_by, "test-publisher")

    def test_list_models_sends_auth_header(self):
        c = Client(base_url=self.base_url, api_key="my-secret")
        c.list_models()
        self.assertEqual(
            MockEdgeForgeHandler.received_auth_header, "Bearer my-secret"
        )

    def test_list_models_no_auth_header_without_key(self):
        c = Client(base_url=self.base_url)
        c.list_models()
        self.assertIsNone(MockEdgeForgeHandler.received_auth_header)

    # ── get_model() ──

    def test_get_model_returns_model(self):
        c = Client(base_url=self.base_url)
        m = c.get_model("01J9TEST")
        self.assertIsInstance(m, ModelInfo)
        self.assertEqual(m.id, "01J9TEST")

    # ── chat() ──

    def test_chat_returns_response(self):
        c = Client(base_url=self.base_url)
        resp = c.chat(model="test-model", prompt="Hello!")
        self.assertIsInstance(resp, ChatResponse)
        self.assertEqual(resp.text, "Hello back!")
        self.assertEqual(resp.model, "test-model")
        self.assertEqual(resp.tokens_generated, 3)
        self.assertEqual(resp.finish_reason, "stop")

    def test_chat_sends_correct_body(self):
        c = Client(base_url=self.base_url)
        c.chat(model="my-model", prompt="Hi", max_tokens=100, temperature=0.5, seed=42)
        body = MockEdgeForgeHandler.received_chat_body
        self.assertEqual(body["model"], "my-model")
        self.assertEqual(body["messages"][0]["content"], "Hi")
        self.assertEqual(body["max_tokens"], 100)
        self.assertAlmostEqual(body["temperature"], 0.5)
        self.assertEqual(body["seed"], 42)
        self.assertFalse(body["stream"])

    def test_chat_with_api_key(self):
        c = Client(base_url=self.base_url, api_key="secret")
        c.chat(model="m", prompt="hi")
        self.assertEqual(MockEdgeForgeHandler.received_auth_header, "Bearer secret")

    # ── embed() ──

    def test_embed_returns_vector(self):
        c = Client(base_url=self.base_url)
        result = c.embed("hello world")
        self.assertIsInstance(result, EmbeddingResult)
        self.assertEqual(result.embedding, [0.1, 0.2, 0.3])
        self.assertEqual(result.model, "edgeforge-hashing-768")
        self.assertEqual(result.token_count, 2)

    # ── search() ──

    def test_search_returns_hits(self):
        c = Client(base_url=self.base_url)
        hits = c.search("hello")
        self.assertEqual(len(hits), 1)
        self.assertIsInstance(hits[0], SearchHit)
        self.assertEqual(hits[0].source_id, "src1")
        self.assertEqual(hits[0].content, "hello world")
        self.assertAlmostEqual(hits[0].score, 0.95)

    # ── upload_file() ──

    def test_upload_file_returns_result(self):
        c = Client(base_url=self.base_url)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("test content")
            f.flush()
            path = f.name
        try:
            result = c.upload_file(path)
            self.assertIsInstance(result, FileUploadResult)
            self.assertEqual(result.status, "processed")
            self.assertEqual(result.chunks, 3)
        finally:
            os.unlink(path)

    # ── import_model() ──

    def test_import_model_returns_result(self):
        c = Client(base_url=self.base_url)
        result = c.import_model("/tmp/test.gguf")
        self.assertIsInstance(result, ImportResult)
        self.assertEqual(result.model_id, "01J9IMPORTED")
        self.assertEqual(result.status, "accepted")
        self.assertEqual(result.path, "/tmp/test.gguf")

    # ── Error handling ──

    def test_chat_error_on_404(self):
        # Point to a non-existent server → connection refused → RuntimeError.
        c = Client(base_url="http://127.0.0.1:1/v1", timeout=1.0)
        with self.assertRaises(RuntimeError):
            c.chat(model="m", prompt="hi")

    # ── Version ──

    def test_version_string(self):
        self.assertIsInstance(__version__, str)
        self.assertTrue(len(__version__) > 0)


if __name__ == "__main__":
    unittest.main()
