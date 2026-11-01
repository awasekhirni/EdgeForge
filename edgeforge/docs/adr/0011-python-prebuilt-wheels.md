# ADR-011: Python SDK as Prebuilt Wheels

**Status:** Approved
**Date:** 2026-10-31

## Decision

The Python SDK ships as prebuilt wheels via `maturin` + `cibuildwheel`:

- **Python versions:** cp39–cp313
- **Platforms:** macOS universal2, manylinux x86_64/aarch64, Windows x64
- **Build:** `pyo3` with `extension-module` feature

## Usage

```bash
pip install edgeforge
```

```python
import edgeforge
client = edgeforge.Client()
print(client.chat(model="my-model", prompt="Hello!").text)
```
