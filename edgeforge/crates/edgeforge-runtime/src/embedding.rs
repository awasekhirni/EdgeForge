//! # Embedding Engine — RAG Vector Embeddings (F-DEV-06)
//!
//! ## Purpose
//! Produces dense vector embeddings for text input, used by the RAG
//! (Retrieval-Augmented Generation) pipeline. The `EmbeddingEngine` trait
//! is the L4 abstraction; `HashingEmbedder` is the pure-Rust default
//! implementation that works out of the box with no external dependencies.
//!
//! ## What This File Does
//! - `EmbeddingEngine` trait — `embed(&str) -> Vec<f32>`, `dim() -> usize`,
//!   `model_id() -> &str`
//! - `HashingEmbedder` — pure-Rust embedder using feature-hashing (the
//!   "hashing trick") over word + character-bigram features. Deterministic,
//!   dependency-free, L2-normalised output.
//! - `cosine_similarity(a, b)` — pure function for similarity scoring
//! - `dot_product(a, b)` — pure function
//! - `l2_norm(a)` — pure function
//! - `encode_vector_f32le` / `decode_vector_f32le` — BLOB serialization for
//!   SQLite storage
//!
//! ## Token Source
//! `HashingEmbedder` is **not** a neural network. It uses the feature-hashing
//! trick: each word and character-bigram in the input is hashed into a
//! fixed-dimensional vector, with sign-hashing to reduce collisions. The
//! output is L2-normalised so cosine similarity = dot product.
//!
//! This is the same design used by Facebook's fastText for its hashing-based
//! fallback mode, and by scikit-learn's `FeatureHasher`. It produces
//! deterministic, real, non-zero vectors that support meaningful similarity
//! search — two texts sharing many words will have high cosine similarity.
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError (for error conversion)
//! - `std::collections::HashMap` — for the signed-feature accumulator
//!
//! ## Output
//! `Vec<f32>` — L2-normalised embedding vector of dimension `dim()`.
//!
//! ## Function: embed
//! - **Purpose:** Produce a dense vector embedding for a text input
//! - **Parameters:** `&str` — the text to embed
//! - **Returns:** `Vec<f32>` — L2-normalised embedding
//! - **Algorithm:**
//!   1. Tokenize the text (lowercase, alphanumeric words + char-bigrams)
//!   2. For each token, hash it to an index in [0, dim) and a sign in {-1, +1}
//!   3. Accumulate sign * weight at each index
//!   4. L2-normalise the resulting vector
//! - **Determinism:** Same input always produces the same vector (no RNG)
//!
//! ## Function: cosine_similarity
//! - **Purpose:** Compute the cosine similarity between two vectors
//! - **Parameters:** `&[f32]`, `&[f32]` — the vectors (must be same length)
//! - **Returns:** `f32` — similarity in [-1, 1]. Returns 0.0 for zero vectors
//!   or mismatched lengths.

use std::collections::HashMap;

// ── Trait ──────────────────────────────────────────────────────────────────

/// The L4 embedding engine abstraction.
///
/// L5 (edgeforge-api / edgeforge-orchestration) talks only to this trait,
/// never to a concrete embedder. This lets us swap `HashingEmbedder` for a
/// real neural embedder (e.g. a GGUF embedding model via LlamaBackend) without
/// touching the API or ORM layer.
pub trait EmbeddingEngine: Send + Sync + 'static {
    /// Produce a dense vector embedding for the given text.
    ///
    /// ## Parameters
    /// - `text`: The text to embed.
    ///
    /// ## Returns
    /// `Vec<f32>` — L2-normalised embedding of dimension [`dim`](Self::dim).
    fn embed(&self, text: &str) -> Vec<f32>;

    /// The dimensionality of the vectors produced by this embedder.
    fn dim(&self) -> usize;

    /// A stable identifier for this embedding model (stored alongside vectors
    /// so queries can filter by model).
    fn model_id(&self) -> &str;
}

// ── HashingEmbedder ────────────────────────────────────────────────────────

/// Configuration for [`HashingEmbedder`].
#[derive(Clone, Debug)]
pub struct HashingEmbedderConfig {
    /// Vector dimensionality. 768 matches many transformer embedding models.
    pub dim: usize,
    /// Whether to include character bigrams as features (helps with
    /// morphology and typos).
    pub use_char_bigrams: bool,
    /// Whether to include word unigrams as features.
    pub use_word_unigrams: bool,
    /// The model ID reported by [`EmbeddingEngine::model_id`].
    pub model_id: String,
}

impl Default for HashingEmbedderConfig {
    fn default() -> Self {
        Self {
            dim: 768,
            use_char_bigrams: true,
            use_word_unigrams: true,
            model_id: "edgeforge-hashing-768".to_string(),
        }
    }
}

/// A pure-Rust embedding engine using the feature-hashing trick.
///
/// Produces deterministic, L2-normalised vectors over word unigrams and
/// character bigrams. No external dependencies, no neural network — but
/// the vectors support meaningful cosine-similarity search because texts
/// that share many tokens hash to the same indices.
///
/// ## When to Use
/// - Default RAG pipeline when no neural embedding model is available
/// - Air-gapped / bootstrap environments
/// - Integration testing
/// - As a fallback when a GGUF embedding model fails to load
pub struct HashingEmbedder {
    config: HashingEmbedderConfig,
}

impl HashingEmbedder {
    /// Create a new `HashingEmbedder` with default configuration
    /// (dim=768, word unigrams + char bigrams).
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HashingEmbedderConfig::default())
    }

    /// Create a new `HashingEmbedder` with custom configuration.
    #[must_use]
    pub fn with_config(config: HashingEmbedderConfig) -> Self {
        tracing::info!(
            dim = config.dim,
            use_char_bigrams = config.use_char_bigrams,
            use_word_unigrams = config.use_word_unigrams,
            model_id = %config.model_id,
            "HashingEmbedder initialized"
        );
        Self { config }
    }

    /// Borrow the embedder's configuration (read-only).
    #[must_use]
    pub fn config(&self) -> &HashingEmbedderConfig {
        &self.config
    }

    /// Tokenize text into word unigrams.
    fn word_unigrams(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty())
            .collect()
    }

    /// Tokenize text into character bigrams (including word boundaries).
    fn char_bigrams(text: &str) -> Vec<String> {
        let lowered = text.to_lowercase();
        let chars: Vec<char> = lowered.chars().collect();
        if chars.len() < 2 {
            return Vec::new();
        }
        let mut bigrams = Vec::with_capacity(chars.len());
        // Leading boundary bigram: "<" + first char
        bigrams.push(format!("<{}", chars[0]));
        for window in chars.windows(2) {
            bigrams.push(format!("{}{}", window[0], window[1]));
        }
        // Trailing boundary bigram: last char + ">"
        bigrams.push(format!("{}>", chars[chars.len() - 1]));
        bigrams
    }

    /// Hash a feature string to a (index, sign) pair.
    ///
    /// Uses FNV-1a 64-bit for the index and a second pass for the sign.
    /// The sign reduces hash collisions' impact: two features colliding
    /// at the same index will, with probability 0.5, have opposite signs
    /// and cancel out rather than reinforce.
    fn hash_feature(feature: &str, dim: usize) -> (usize, f32) {
        let h1 = fnv1a64(feature.as_bytes());
        let index = (h1 % dim as u64) as usize;
        // Use the high bit of a second hash for the sign.
        let h2 = fnv1a64_seed(feature.as_bytes(), h1 ^ 0x9E3779B97F4A7C15);
        let sign = if h2 & 1 == 0 { -1.0 } else { 1.0 };
        (index, sign)
    }
}

impl Default for HashingEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingEngine for HashingEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let dim = self.config.dim;
        let mut accum: HashMap<usize, f32> = HashMap::with_capacity(dim);

        // Word unigrams (weight 1.0 each).
        if self.config.use_word_unigrams {
            for word in Self::word_unigrams(text) {
                let (idx, sign) = Self::hash_feature(&word, dim);
                *accum.entry(idx).or_insert(0.0) += sign;
            }
        }

        // Character bigrams (weight 0.5 each — less important than words).
        if self.config.use_char_bigrams {
            for bg in Self::char_bigrams(text) {
                let (idx, sign) = Self::hash_feature(&bg, dim);
                *accum.entry(idx).or_insert(0.0) += sign * 0.5;
            }
        }

        // Build the dense vector.
        let mut vec = vec![0.0f32; dim];
        for (idx, val) in accum {
            vec[idx] = val;
        }

        // L2-normalise.
        let norm = l2_norm(&vec);
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        tracing::trace!(
            text_len = text.len(),
            dim,
            nonzero = vec.iter().filter(|v| v.abs() > 1e-9).count(),
            "HashingEmbedder: embedded text"
        );

        vec
    }

    fn dim(&self) -> usize {
        self.config.dim
    }

    fn model_id(&self) -> &str {
        &self.config.model_id
    }
}

// ── Vector math utilities ──────────────────────────────────────────────────

/// Compute the L2 (Euclidean) norm of a vector.
///
/// ## Parameters
/// - `v`: The input vector.
///
/// ## Returns
/// `f32` — sqrt(sum of squares). Returns 0.0 for empty or all-zero vectors.
#[must_use]
pub fn l2_norm(v: &[f32]) -> f32 {
    let sum_sq: f64 = v.iter().map(|x| (*x as f64) * (*x as f64)).sum();
    (sum_sq.sqrt()) as f32
}

/// Compute the dot product of two equal-length vectors.
///
/// ## Parameters
/// - `a`, `b`: The input vectors.
///
/// ## Returns
/// `f32` — sum of elementwise products. Returns 0.0 for mismatched lengths.
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += (*x as f64) * (*y as f64);
    }
    sum as f32
}

/// Compute the cosine similarity between two vectors.
///
/// ## Parameters
/// - `a`, `b`: The input vectors (need not be normalised).
///
/// ## Returns
/// `f32` — similarity in [-1, 1]. Returns 0.0 for zero vectors or
/// mismatched lengths.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot_product(a, b) / (norm_a * norm_b)
}

// ── BLOB serialization ─────────────────────────────────────────────────────

/// Encode a vector as little-endian f32 bytes for SQLite BLOB storage.
///
/// ## Parameters
/// - `v`: The vector to encode.
///
/// ## Returns
/// `Vec<u8>` — 4 bytes per element, little-endian.
#[must_use]
pub fn encode_vector_f32le(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &val in v {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Decode a little-endian f32 byte slice into a vector.
///
/// ## Parameters
/// - `bytes`: The BLOB data (must be a multiple of 4 bytes).
///
/// ## Returns
/// `Vec<f32>` — the decoded vector. Returns an empty vec if the byte count
/// is not a multiple of 4.
#[must_use]
pub fn decode_vector_f32le(bytes: &[u8]) -> Vec<f32> {
    if bytes.len() % 4 != 0 {
        tracing::warn!(
            byte_count = bytes.len(),
            "decode_vector_f32le: byte count is not a multiple of 4"
        );
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0, 0, 0, 0]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ── Internal: FNV-1a hashing ───────────────────────────────────────────────

/// FNV-1a 64-bit hash.
fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_seed(bytes, 0xcbf29ce484222325)
}

/// FNV-1a 64-bit hash with a custom initial offset (for sign derivation).
fn fnv1a64_seed(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = seed;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedder_produces_correct_dimension() {
        let e = HashingEmbedder::with_config(HashingEmbedderConfig {
            dim: 256,
            ..HashingEmbedderConfig::default()
        });
        let v = e.embed("hello world");
        assert_eq!(v.len(), 256);
    }

    #[test]
    fn embedder_produces_nonzero_vector() {
        let e = HashingEmbedder::new();
        let v = e.embed("the quick brown fox");
        assert!(v.iter().any(|x| x.abs() > 1e-9), "vector should be non-zero");
    }

    #[test]
    fn embedder_output_is_l2_normalised() {
        let e = HashingEmbedder::new();
        let v = e.embed("the quick brown fox jumps over the lazy dog");
        let norm = l2_norm(&v);
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "L2 norm should be ~1.0, got {norm}"
        );
    }

    #[test]
    fn embedder_is_deterministic() {
        let e = HashingEmbedder::new();
        let v1 = e.embed("hello world from edgeforge");
        let v2 = e.embed("hello world from edgeforge");
        assert_eq!(v1, v2, "same input must produce identical output");
    }

    #[test]
    fn similar_texts_have_high_similarity() {
        let e = HashingEmbedder::new();
        let v1 = e.embed("the quick brown fox jumps over the lazy dog");
        let v2 = e.embed("the quick brown fox jumps over the lazy cat");
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            sim > 0.8,
            "texts sharing most words should have high similarity, got {sim}"
        );
    }

    #[test]
    fn dissimilar_texts_have_low_similarity() {
        let e = HashingEmbedder::new();
        let v1 = e.embed("the quick brown fox jumps over the lazy dog");
        let v2 = e.embed("abcdefghijklmnopqrstuvwxyz 1234567890");
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            sim < 0.5,
            "texts sharing few words should have low similarity, got {sim}"
        );
    }

    #[test]
    fn empty_text_produces_zero_vector() {
        let e = HashingEmbedder::new();
        let v = e.embed("");
        assert!(v.iter().all(|x| x.abs() < 1e-9), "empty text → zero vector");
    }

    #[test]
    fn cosine_similarity_handles_mismatched_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_handles_zero_vectors() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_identical_vectors_is_one() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-4, "identical vectors → sim=1.0, got {sim}");
    }

    #[test]
    fn dot_product_basic() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        assert_eq!(dot_product(&a, &b), 32.0); // 4+10+18
    }

    #[test]
    fn encode_decode_roundtrip() {
        let v = vec![0.1, -0.2, 0.3, -0.4, 0.5];
        let bytes = encode_vector_f32le(&v);
        assert_eq!(bytes.len(), v.len() * 4);
        let decoded = decode_vector_f32le(&bytes);
        assert_eq!(decoded.len(), v.len());
        for (orig, dec) in v.iter().zip(decoded.iter()) {
            assert!((orig - dec).abs() < 1e-6, "roundtrip should preserve values");
        }
    }

    #[test]
    fn decode_rejects_invalid_byte_count() {
        let bytes = [0u8; 5]; // not a multiple of 4
        let v = decode_vector_f32le(&bytes);
        assert!(v.is_empty(), "invalid byte count → empty vec");
    }

    #[test]
    fn char_bigrams_include_boundaries() {
        let bgs = HashingEmbedder::char_bigrams("hi");
        assert!(bgs.contains(&"<h".to_string()), "should have leading boundary");
        assert!(bgs.contains(&"hi".to_string()), "should have interior bigram");
        assert!(bgs.contains(&"i>".to_string()), "should have trailing boundary");
    }

    #[test]
    fn word_unigrams_strips_punctuation() {
        let words = HashingEmbedder::word_unigrams("Hello, World! Don't.");
        assert_eq!(words, vec!["hello", "world", "don't"]);
    }

    #[test]
    fn model_id_is_configurable() {
        let e = HashingEmbedder::with_config(HashingEmbedderConfig {
            model_id: "my-custom-embedder".to_string(),
            ..HashingEmbedderConfig::default()
        });
        assert_eq!(e.model_id(), "my-custom-embedder");
    }

    #[test]
    fn disable_char_bigrams() {
        let e = HashingEmbedder::with_config(HashingEmbedderConfig {
            use_char_bigrams: false,
            ..HashingEmbedderConfig::default()
        });
        let v = e.embed("hello world");
        // Should still produce a non-zero, normalised vector.
        assert!(v.iter().any(|x| x.abs() > 1e-9));
        let norm = l2_norm(&v);
        assert!((norm - 1.0).abs() < 1e-4);
    }
}
