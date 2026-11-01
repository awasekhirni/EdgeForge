// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
// wrapper.h — bindgen input for llama.cpp C API.
//
// This file is processed by bindgen (via build.rs) to generate Rust FFI
// bindings for the llama.cpp C API. Only the functions/types we actually
// use are allowlisted in build.rs.
//
// When the `llama-backend` feature is disabled OR llama.cpp is not built,
// the build.rs falls back to stub mode (cfg=llama_stub) and this file is
// not processed.

#include "llama.h"
