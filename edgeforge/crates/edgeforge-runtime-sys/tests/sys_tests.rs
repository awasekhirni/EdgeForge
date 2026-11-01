//! # Runtime Sys Tests — edgeforge-runtime-sys
//!
//! Tests for: stub mode behavior, FfiError display, is_available.

#[cfg(test)]
mod tests {
    use edgeforge_runtime_sys::*;

    #[test]
    fn test_stub_returns_not_implemented() {
        // In stub mode, load_model should return NotImplemented
        let path = std::path::Path::new("/nonexistent/model.gguf");
        let result = load_model(path, &LlamaModelParams::default());
        assert!(result.is_err());
        match result {
            Err(FfiError::NotImplemented(_)) => {}, // expected
            _ => panic!("expected NotImplemented error"),
        }
    }

    #[test]
    fn test_ffi_error_display() {
        let e = FfiError::NotImplemented("test");
        assert!(format!("{e}").contains("not implemented"));
        let e = FfiError::Ffi("test error".to_string());
        assert!(format!("{e}").contains("FFI error"));
        let e = FfiError::NullPointer;
        assert!(format!("{e}").contains("null pointer"));
        let e = FfiError::InvalidArgument("bad arg".to_string());
        assert!(format!("{e}").contains("invalid argument"));
    }

    #[test]
    fn test_is_available_stub() {
        // In stub mode, is_available should return false
        assert!(!is_available());
    }

    #[test]
    fn test_backend_info() {
        let info = backend_info();
        assert!(!info.is_empty());
    }

    #[test]
    fn test_model_params_default() {
        let p = LlamaModelParams::default();
        assert_eq!(p.n_ctx, 4096);
        assert_eq!(p.n_gpu_layers, 0);
        assert!(p.use_mmap);
        assert!(!p.use_mlock);
    }

    #[test]
    fn test_generation_params_default() {
        let p = LlamaGenerationParams::default();
        assert_eq!(p.n_predict, 512);
        assert_eq!(p.temp, 0.7);
        assert_eq!(p.top_p, 0.9);
        assert_eq!(p.seed, -1);
    }
}
