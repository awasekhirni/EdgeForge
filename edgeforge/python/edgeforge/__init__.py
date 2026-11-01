"""EdgeForge Python SDK.

Native client for the EdgeForge inference platform. Built with PyO3 for
high-performance access to the EdgeForge REST API.

## Quick Start

```python
import edgeforge

client = edgeforge.Client("http://127.0.0.1:11434/v1", api_key="my-secret")
print(client.health().status)  # "ok"

resp = client.chat(model="my-model", prompt="Hello!")
print(resp.text)
```

## Module Contents

- `Client` — the main client class
- `ChatResponse`, `ModelInfo`, `HealthStatus`, `ImportResult`,
  `EmbeddingResult`, `SearchHit`, `FileUploadResult` — response types
- `__version__` — the SDK version string

## Fallback

If the compiled native extension is unavailable (e.g. the wheel wasn't
installed for your platform), a pure-Python fallback is used that wraps
`urllib.request`. The fallback has the same API but is slower.
"""

# Try to import the compiled native extension. If it's unavailable (e.g. the
# wheel wasn't installed for this platform, or the user is running from
# source without `maturin develop`), fall back to the pure-Python
# implementation.
try:
    from .edgeforge_python import (  # type: ignore
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

    # Mark that we're using the native extension.
    _NATIVE = True
except ImportError:
    # Fall back to the pure-Python implementation.
    from ._fallback import (  # type: ignore
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

    # Mark that we're using the fallback.
    _NATIVE = False

__all__ = [
    "Client",
    "ChatResponse",
    "ModelInfo",
    "HealthStatus",
    "ImportResult",
    "EmbeddingResult",
    "SearchHit",
    "FileUploadResult",
    "__version__",
    "_NATIVE",
]
