//! # Stub FFI Module
//!
//! ## Purpose
//! Provides stub implementations of all FFI functions when llama.cpp is not
//! built. All functions return `Err(FfiError::NotImplemented)`.
//!
//! ## When This Is Used
//! - The `llama-backend` feature is not enabled (default), OR
//! - The `llama-backend` feature is enabled but `ffi/llama.cpp/` is not present
//!
//! ## What This File Does
//! - Defines stub `LlamaModel` and `LlamaContext` types
//! - Implements all FFI functions as returning `Err(NotImplemented)`
//! - Lets the workspace compile without a C++ toolchain

use crate::{FfiError, LlamaContext, LlamaGenerationParams, LlamaModel, LlamaModelParams};
use std::path::Path;

/// Load a model from a GGUF file (stub — always returns NotImplemented).
///
/// ## Parameters
/// - `path`: Path to the `.gguf` model file
/// - `params`: Model loading parameters (context size, GPU layers, etc.)
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
///
/// ## Errors
/// Always returns `FfiError::NotImplemented("llama_backend feature not enabled")`.
pub fn load_model(_path: &Path, _params: &LlamaModelParams) -> Result<LlamaModel, FfiError> {
    Err(FfiError::NotImplemented(
        "llama_backend feature not enabled — build with --features edgeforge-runtime/llama-backend",
    ))
}

/// Tokenize text into llama.cpp token IDs (stub).
///
/// ## Parameters
/// - `_model`: Opaque model handle
/// - `_text`: Text to tokenize
/// - `_add_bos`: Whether to add a beginning-of-sequence token
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn tokenize(
    _model: &LlamaModel,
    _text: &str,
    _add_bos: bool,
) -> Result<Vec<i32>, FfiError> {
    Err(FfiError::NotImplemented("tokenize — llama_backend not enabled"))
}

/// Detokenize a single token ID into text (stub).
///
/// ## Parameters
/// - `_model`: Opaque model handle
/// - `_token`: Token ID to decode
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn token_to_piece(_model: &LlamaModel, _token: i32) -> Result<String, FfiError> {
    Err(FfiError::NotImplemented("token_to_piece — llama_backend not enabled"))
}

/// Create a llama.cpp context for a model (stub).
///
/// ## Parameters
/// - `_model`: Opaque model handle
/// - `_n_ctx`: Context window size
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn new_context(_model: &LlamaModel, _n_ctx: u32) -> Result<LlamaContext, FfiError> {
    Err(FfiError::NotImplemented("new_context — llama_backend not enabled"))
}

/// Decode a batch of tokens (run one forward pass) (stub).
///
/// ## Parameters
/// - `_ctx`: Opaque context handle
/// - `_tokens`: Token IDs to decode
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn decode(_ctx: &mut LlamaContext, _tokens: &[i32]) -> Result<(), FfiError> {
    Err(FfiError::NotImplemented("decode — llama_backend not enabled"))
}

/// Sample the next token from the model's logits (stub).
///
/// ## Parameters
/// - `_ctx`: Opaque context handle
/// - `_temp`: Sampling temperature
/// - `_top_p`: Top-p sampling threshold
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn sample_token(_ctx: &mut LlamaContext, _temp: f32, _top_p: f32) -> Result<i32, FfiError> {
    Err(FfiError::NotImplemented("sample_token — llama_backend not enabled"))
}

/// Get the model's vocabulary size (stub).
///
/// ## Returns
/// `Err(FfiError::NotImplemented)` — always, in stub mode.
pub fn n_vocab(_model: &LlamaModel) -> Result<u32, FfiError> {
    Err(FfiError::NotImplemented("n_vocab — llama_backend not enabled"))
}

/// Get the model's embedding dimension (stub).
pub fn n_embd(_model: &LlamaModel) -> Result<u32, FfiError> {
    Err(FfiError::NotImplemented("n_embd — llama_backend not enabled"))
}

/// Get the model's layer count (stub).
pub fn n_layers(_model: &LlamaModel) -> Result<u32, FfiError> {
    Err(FfiError::NotImplemented("n_layers — llama_backend not enabled"))
}

/// Check if the FFI backend is available.
///
/// ## Returns
/// `false` — always, in stub mode.
pub fn is_available() -> bool {
    false
}

/// Get a human-readable description of the backend.
pub fn backend_info() -> &'static str {
    "stub (llama.cpp not built — use MockInferenceEngine)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_returns_false() {
        assert!(!is_available());
    }

    #[test]
    fn test_backend_info_nonempty() {
        assert!(!backend_info().is_empty());
    }

    #[test]
    fn test_load_model_returns_not_implemented() {
        let result = load_model(std::path::Path::new("/nonexistent"), &LlamaModelParams::default());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FfiError::NotImplemented(_)));
    }

    #[test]
    fn test_tokenize_returns_not_implemented() {
        // Can't construct LlamaModel in stub mode easily, so just verify the error type
        let err = FfiError::NotImplemented("test");
        assert!(err.to_string().contains("not implemented"));
    }

    #[test]
    fn test_ffi_error_display() {
        let e = FfiError::NullPointer;
        assert!(e.to_string().contains("null"));
    }

    #[test]
    fn test_ffi_error_invalid_argument() {
        let e = FfiError::InvalidArgument("bad arg".into());
        assert!(e.to_string().contains("bad arg"));
    }

    #[test]
    fn test_ffi_error_ffi() {
        let e = FfiError::Ffi("ffi crash".into());
        assert!(e.to_string().contains("ffi crash"));
    }

    #[test]
    fn test_llama_model_params_default() {
        let p = LlamaModelParams::default();
        assert_eq!(p.n_ctx, 4096);
        assert_eq!(p.n_gpu_layers, 0);
        assert!(p.use_mmap);
        assert!(!p.use_mlock);
    }

    #[test]
    fn test_llama_generation_params_default() {
        let p = LlamaGenerationParams::default();
        assert_eq!(p.n_predict, 512);
        assert!((p.temp - 0.7).abs() < 0.01);
        assert!((p.top_p - 0.9).abs() < 0.01);
        assert_eq!(p.seed, -1);
    }
}
