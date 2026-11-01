// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Observability Tests — edgeforge-observability
//!
//! Tests for: subscriber initialization, idempotency.

#[cfg(test)]
mod tests {
    use edgeforge_core::Config;
    use edgeforge_observability::init;

    #[test]
    fn test_init_does_not_panic() {
        let cfg = Config::default();
        let _guard1 = init(&cfg);
        // Should not panic on second call (idempotent via OnceLock)
        let _guard2 = init(&cfg);
    }
}
