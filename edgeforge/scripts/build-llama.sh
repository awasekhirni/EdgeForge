# // Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
# // Author: Awase Khirni Syed
#!/usr/bin/env bash
set -euo pipefail
echo "[build-llama] Phase 3+ — builds llama.cpp for the host triple"
echo "[build-llama] Run: git submodule add https://github.com/ggerganov/llama.cpp ffi/llama.cpp"
echo "[build-llama] Then: cd ffi/llama.cpp && mkdir build && cd build && cmake .. && make -j"
