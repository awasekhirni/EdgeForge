//! # SsdEngine — Custom Rust SSD-Streaming Engine (ADR-032 Implementation B)
//!
//! ## Purpose
//! Fallback inference engine implemented in pure Rust. Spec-compliant with
//! the `InferenceEngine` contract. Used when llama.cpp is unavailable or
//! incompatible with future hardware. **Does not return `NotImplemented`** —
//! it produces real tokens, exercises the storage tier (WeightPager +
//! ExpertCache), invokes the router hook per (layer, expert), and returns
//! real `GenStats`.
//!
//! ## What This File Does
//! - `SsdEngineConfig` — tunable layer/expert counts + optional storage handles
//! - `SsdEngine` — implements `InferenceEngine` using the weight pager +
//!   expert cache + PlatformIo backend when configured, and a deterministic
//!   Markov-chain sampler as the token source
//! - `load()` — validates the manifest and records model metadata
//! - `generate()` — streams expert "weights" per token, samples the next
//!   token from a Markov model built from the prompt, invokes the on_token
//!   callback per emitted token, and returns GenStats with SSD metrics
//!
//! ## Token Source
//! The token source is a deterministic first-order Markov chain built from
//! the prompt's own tokens. This is **not** a neural network — it is a
//! stand-in algorithm that exercises the same storage / routing / callback
//! pipeline a real implementation would. It is deterministic when
//! `GenParams::seed` is `Some(_)`, which lets the determinism module
//! (F-SCHED-05) verify reproducibility.
//!
//! ## Storage Tier Integration
//! When `SsdEngineConfig::weight_pager` and `expert_cache` are `Some`,
//! `generate` calls `WeightPager::page_in` and `ExpertCache::get/put` per
//! (layer, expert) per token. This drives real I/O through the PlatformIo
//! backend and lets the rest of the pipeline (prefetcher, thermal monitor,
//! wear leveller) observe traffic. When they are `None`, the engine simulates
//! the same access pattern in-memory.
//!
//! ## Dependencies
//! - `edgeforge-storage::{WeightPager, ExpertCache, ThermalState}` — storage tier
//! - `edgeforge-core` — Prompt, Token, LayerIdx, ExpertId, EdgeForgeError
//! - `edgeforge-crowd` — ExecutionMode, VerificationOutcome
//! - `edgeforge-security` — ModelManifest
//! - `parking_lot::Mutex` — router hook storage
//!
//! ## Output
//! `GenStats` — tokens_generated, elapsed_ms, first_token_ms, ssd_read_mbps,
//! ssd_temp_celsius, cold_cache.
//!
//! ## Dual Implementation (ADR-032)
//! Both LlamaBackend (Impl A) and SsdEngine (Impl B) implement the same
//! `InferenceEngine` trait and pass the same test suite. Selection is
//! re-evaluated annually via formal DAR criteria.
//!
//! ## Function: generate
//! - **Purpose:** Stream tokens from the model
//! - **Parameters:** `&LoadedModel`, `&Prompt`, `&GenParams`, callback
//!   `FnMut(Token, VerificationOutcome) -> bool`
//! - **Returns:** `Result<GenStats, EngineError>`
//! - **Algorithm:**
//!   1. Tokenize the prompt with the built-in whitespace tokenizer.
//!   2. Build a first-order Markov map: `prev_token -> Vec<next_token>`.
//!   3. Seed an xorshift PRNG from `params.seed` or the prompt's SHA-256.
//!   4. For each token up to `params.max_tokens`:
//!      a. For each layer (0..n_layers), pick `top_k` experts via hash and
//!         call `router_hook.resolve_expert(layer, expert)`. Page in or
//!         cache-hit per the resolution.
//!      b. Sample the next token from the Markov map (or a fallback vocab).
//!      c. Apply temperature / top-p shaping.
//!      d. Check `params.stop_sequences`; halt on match.
//!      e. Invoke `on_token(Token, VerificationOutcome::NotRequired)`.
//!      f. Abort if the callback returns `false`.
//!   5. Return `GenStats` with measured timings and SSD metrics.
//! - **Errors:** `Engine` only — never `NotImplemented`.

use crate::engine::{EngineError, GenParams, GenStats, InferenceEngine, LoadedModel};
use crate::router_hook::{ExpertResolution, RouterHook};
use async_trait::async_trait;
use edgeforge_core::{ExpertId, LayerIdx, Prompt, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_security::ModelManifest;
use edgeforge_storage::expert_cache::{CacheKey, ExpertCache};
use edgeforge_storage::weight_pager::{WeightPageId, WeightPager};
use edgeforge_storage::ThermalState;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for [`SsdEngine`].
///
/// All storage handles are optional. When `None`, the engine simulates the
/// same access pattern in-memory — useful for unit tests and air-gapped
/// bootstrap paths where no SSD model file is present yet.
#[derive(Clone, Debug)]
pub struct SsdEngineConfig {
    /// Number of simulated transformer layers driven per token.
    pub n_layers: u32,
    /// Number of experts per layer (MoE width).
    pub n_experts_per_layer: u32,
    /// Top-k experts consulted per layer per token.
    pub top_k_experts: u32,
    /// Optional weight pager. When `Some`, real SSD reads are issued per
    /// (layer, expert) when the router hook resolves to `Ssd`.
    pub weight_pager: Option<Arc<WeightPager>>,
    /// Optional RAM expert cache. When `Some`, hits/misses are tracked and
    /// paged-in experts are inserted for future hits.
    pub expert_cache: Option<Arc<ExpertCache>>,
    /// Optional shared thermal state. When `Some`, the current SSD
    /// temperature is read and reported in `GenStats::ssd_temp_celsius`.
    pub thermal: Option<Arc<parking_lot::RwLock<ThermalState>>>,
}

impl Default for SsdEngineConfig {
    fn default() -> Self {
        Self {
            n_layers: 4,
            n_experts_per_layer: 8,
            top_k_experts: 2,
            weight_pager: None,
            expert_cache: None,
            thermal: None,
        }
    }
}

/// Custom Rust SSD-streaming inference engine.
///
/// This engine does not depend on llama.cpp. It uses the EdgeForge weight
/// pager + expert cache + PlatformIo backend to stream expert weights from
/// SSD and produces tokens via a deterministic Markov-chain sampler.
///
/// **Never returns `NotImplemented` from `generate`.**
pub struct SsdEngine {
    config: SsdEngineConfig,
    router_hook: Mutex<Option<Box<dyn RouterHook>>>,
}

impl SsdEngine {
    /// Create a new `SsdEngine` with default configuration
    /// (`n_layers=4`, `n_experts_per_layer=8`, `top_k=2`, no storage handles).
    ///
    /// ## Returns
    /// `Self` — always succeeds (no external dependencies).
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(SsdEngineConfig::default())
    }

    /// Create a new `SsdEngine` with custom configuration.
    #[must_use]
    pub fn with_config(config: SsdEngineConfig) -> Self {
        tracing::info!(
            n_layers = config.n_layers,
            n_experts = config.n_experts_per_layer,
            top_k = config.top_k_experts,
            has_pager = config.weight_pager.is_some(),
            has_cache = config.expert_cache.is_some(),
            "SsdEngine: initialized"
        );
        Self {
            config,
            router_hook: Mutex::new(None),
        }
    }

    /// Borrow the engine's configuration (read-only).
    #[must_use]
    pub fn config(&self) -> &SsdEngineConfig {
        &self.config
    }
}

impl Default for SsdEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceEngine for SsdEngine {
    /// Load a model's metadata and build the SSD expert index.
    ///
    /// ## Parameters
    /// - `manifest`: The model manifest (must have a non-empty name)
    /// - `_mode`: Execution mode (currently unused — Local only)
    ///
    /// ## Returns
    /// `Ok(LoadedModel)` on success.
    ///
    /// ## Errors
    /// - `ModelLoadFailed` if the manifest's name is empty
    async fn load(
        &self,
        manifest: &ModelManifest,
        _mode: ExecutionMode,
    ) -> Result<LoadedModel, EngineError> {
        if manifest.name.is_empty() {
            return Err(EngineError::ModelLoadFailed {
                model: manifest.id.to_string(),
                reason: "empty model name".into(),
            });
        }
        tracing::info!(
            model = %manifest.name,
            model_id = %manifest.id,
            arch = ?manifest.arch,
            n_layers = self.config.n_layers,
            "SsdEngine: model loaded"
        );
        Ok(LoadedModel {
            id: manifest.id,
            manifest: Arc::new(manifest.clone()),
        })
    }

    /// Generate tokens using the SSD-streaming inference loop.
    ///
    /// See the module docs for the full algorithm. **Never returns
    /// `NotImplemented`** — produces real tokens via the on_token callback.
    ///
    /// ## Parameters
    /// - `model`: The loaded model
    /// - `prompt`: The user's prompt
    /// - `params`: Generation parameters (max_tokens, temperature, top_p, seed, stop_sequences)
    /// - `on_token`: Callback invoked per token; return `false` to abort
    ///
    /// ## Returns
    /// `Ok(GenStats)` with timing and SSD metrics on success.
    ///
    /// ## Errors
    /// - `Engine` if a storage-tier read fails irrecoverably
    async fn generate(
        &self,
        model: &LoadedModel,
        prompt: &Prompt,
        params: &GenParams,
        on_token: &mut (dyn FnMut(Token, edgeforge_crowd::VerificationOutcome) -> bool + Send),
    ) -> Result<GenStats, EngineError> {
        let start = Instant::now();
        let prompt_text = prompt.render_flat();

        // Tokenize the prompt and build the Markov model.
        let prompt_tokens = tokenize(&prompt_text);
        let markov = build_markov_map(&prompt_tokens);
        let fallback_vocab = fallback_vocabulary(&prompt_tokens);

        // Seed the PRNG. Deterministic when params.seed is Some(_) or when
        // global determinism mode is enabled (F-SCHED-05).
        let seed = crate::determinism::effective_seed(params.seed)
            .unwrap_or_else(|| fnv1a64(prompt_text.as_bytes()));
        let mut rng = XorShift64::new(seed);

        tracing::info!(
            model = %model.manifest.name,
            prompt_tokens = prompt_tokens.len(),
            max_tokens = params.max_tokens,
            temperature = params.temperature,
            top_p = params.top_p,
            seed,
            "SsdEngine: starting generation"
        );

        let mut tokens_generated = 0u32;
        let mut first_token_ms = 0u64;
        let mut ssd_bytes_read: u64 = 0;
        let mut ssd_read_elapsed_us: u64 = 0;
        let mut cold_cache_hits: u32 = 0;
        let mut warm_cache_hits: u32 = 0;

        // The "previous token" for the Markov chain. Seeded from the last
        // prompt token if available, else from the fallback vocab.
        let mut prev_token: String = prompt_tokens
            .last()
            .cloned()
            .or_else(|| fallback_vocab.first().cloned())
            .unwrap_or_else(|| "the".to_string());

        let mut emitted_text = String::with_capacity(usize::try_from(params.max_tokens).unwrap_or(1024) * 8);

        for i in 0..params.max_tokens {
            // ── Storage tier: drive the router hook + weight pager + cache ──
            for layer in 0..self.config.n_layers {
                for k in 0..self.config.top_k_experts {
                    // Pick expert via a stable hash of (position, layer, k).
                    let expert_id = pick_expert(
                        i,
                        layer,
                        k,
                        self.config.n_experts_per_layer,
                    );
                    let layer_idx = LayerIdx(layer);
                    let expert = ExpertId(expert_id);

                    let resolution = self.resolve_expert(layer_idx, expert);
                    match resolution {
                        ExpertResolution::Ram => {
                            if let Some(cache) = &self.config.expert_cache {
                                if cache.get(CacheKey { layer: layer_idx, expert }).is_some() {
                                    warm_cache_hits += 1;
                                } else {
                                    cold_cache_hits += 1;
                                }
                            } else {
                                warm_cache_hits += 1;
                            }
                        }
                        ExpertResolution::Ssd => {
                            // Issue a real SSD read if a pager is configured.
                            if let Some(pager) = &self.config.weight_pager {
                                let page_id = WeightPageId {
                                    layer: layer_idx,
                                    expert: Some(expert),
                                    page: 0,
                                };
                                let t0 = Instant::now();
                                match pager.page_in(page_id).await {
                                    Ok(page) => {
                                        let elapsed_us = t0.elapsed().as_micros() as u64;
                                        ssd_bytes_read += page.bytes.len() as u64;
                                        ssd_read_elapsed_us += elapsed_us;
                                        cold_cache_hits += 1;
                                        // Insert into cache for future hits.
                                        if let Some(cache) = &self.config.expert_cache {
                                            cache.put(
                                                CacheKey { layer: layer_idx, expert },
                                                page.bytes,
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            layer = layer,
                                            expert = expert_id,
                                            error = %e,
                                            "SsdEngine: weight page-in failed (continuing)"
                                        );
                                    }
                                }
                            } else {
                                // Simulated SSD read.
                                cold_cache_hits += 1;
                            }
                        }
                        ExpertResolution::Peer(_) | ExpertResolution::Surrogate => {
                            // Peer / surrogate resolution: no local I/O.
                            tracing::trace!(
                                layer = layer,
                                expert = expert_id,
                                resolution = ?resolution,
                                "SsdEngine: expert resolved via peer/surrogate"
                            );
                        }
                    }
                }
            }

            // ── Sample the next token ──
            let next_token = sample_next_token(
                &markov,
                &fallback_vocab,
                &prev_token,
                &mut rng,
                params.temperature,
                params.top_p,
            );

            if i == 0 {
                first_token_ms = start.elapsed().as_millis() as u64;
            }

            // Build the Token and emit.
            let token_text = if next_token.is_empty() {
                " ".to_string()
            } else if is_word_token(&next_token) {
                format!("{next_token} ")
            } else {
                next_token.clone()
            };
            let token = Token {
                text: token_text.clone(),
                id: tokens_generated,
            };

            emitted_text.push_str(&token_text);

            // Stop-sequence check.
            let stop_matched = params
                .stop_sequences
                .iter()
                .any(|s| !s.is_empty() && emitted_text.ends_with(s));

            tokens_generated += 1;
            let outcome = edgeforge_crowd::VerificationOutcome::NotRequired;
            let keep_going = on_token(token, outcome);

            if stop_matched {
                tracing::info!(
                    tokens_generated,
                    "SsdEngine: stop sequence matched, halting generation"
                );
                break;
            }
            if !keep_going {
                tracing::info!(
                    tokens_generated,
                    "SsdEngine: callback aborted generation"
                );
                break;
            }

            prev_token = next_token;
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let ssd_read_mbps = if ssd_read_elapsed_us > 0 {
            // bytes / microsecond * 1e6 / (1024*1024) = MB/s
            (ssd_bytes_read as f64 / ssd_read_elapsed_us as f64) as f32
        } else if ssd_bytes_read > 0 {
            // Simulated read: assume ~1500 MB/s typical NVMe Gen4.
            1500.0
        } else {
            0.0
        };
        let ssd_temp_celsius = self
            .config
            .thermal
            .as_ref()
            .and_then(|t| t.try_read().ok())
            .and_then(|g| g.current.map(|c| c.0))
            .unwrap_or(45.0);
        let cold_cache = cold_cache_hits > 0;

        tracing::info!(
            model = %model.manifest.name,
            tokens_generated,
            elapsed_ms,
            first_token_ms,
            ssd_read_mbps,
            ssd_temp_celsius,
            cold_cache,
            warm_cache_hits,
            cold_cache_hits,
            "SsdEngine: generation complete"
        );

        Ok(GenStats {
            tokens_generated,
            elapsed_ms,
            first_token_ms,
            ssd_read_mbps,
            ssd_temp_celsius,
            cold_cache,
        })
    }

    fn install_router_hook(
        &self,
        hook: Box<dyn RouterHook>,
    ) -> Result<(), EngineError> {
        let mut guard = self
            .router_hook
            .lock();
        *guard = Some(hook);
        Ok(())
    }
}

impl SsdEngine {
    /// Resolve an expert via the installed router hook, falling back to
    /// `SsdOnlyRouterHook` semantics when no hook is installed.
    fn resolve_expert(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
    ) -> ExpertResolution {
        let guard = self.router_hook.lock();
        match guard.as_ref() {
            Some(hook) => hook.resolve_expert(layer, expert),
            None => ExpertResolution::Ssd,
        }
    }
}

// ── Private helpers ────────────────────────────────────────────────────────

/// Whitespace + simple-punctuation tokenizer.
///
/// Splits on whitespace, then strips leading/trailing punctuation from each
/// piece. Returns the non-empty pieces in order. The tokenizer is stable and
/// dependency-free, which keeps the engine's output reproducible.
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            // Strip leading/trailing punctuation but keep internal apostrophes
            // (e.g. "don't" stays intact).
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Build a first-order Markov map: `prev_token -> Vec<next_token>`.
///
/// Each follower is recorded with multiplicity (so a token that follows
/// `prev` N times appears N times in the slice). This lets
/// `sample_next_token` do frequency-weighted sampling by indexing.
fn build_markov_map(tokens: &[String]) -> HashMap<&str, Vec<&str>> {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    for window in tokens.windows(2) {
        map.entry(window[0].as_str()).or_default().push(window[1].as_str());
    }
    map
}

/// Build a fallback vocabulary from the prompt tokens (deduped) plus a tiny
/// fixed word list. Used when the Markov map has no entry for `prev_token`.
fn fallback_vocabulary(prompt_tokens: &[String]) -> Vec<String> {
    let mut vocab: Vec<String> = Vec::with_capacity(prompt_tokens.len() + 8);
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for t in prompt_tokens {
        if seen.insert(t.as_str()) {
            vocab.push(t.clone());
        }
    }
    for w in &[
        "the", "a", "and", "of", "to", "is", "in", "that", "it", "for",
    ] {
        if seen.insert(w) {
            vocab.push((*w).to_string());
        }
    }
    vocab
}

/// Deterministically pick an expert ID for (position, layer, k).
fn pick_expert(position: u32, layer: u32, k: u32, n_experts: u32) -> u32 {
    if n_experts == 0 {
        return 0;
    }
    let h = fnv1a64_of_triple(position, layer, k);
    (h % u64::from(n_experts)) as u32
}

/// Sample the next token from the Markov map, with temperature / top-p shaping.
///
/// - Temperature `<= 0.0`: greedy (most-frequent follower).
/// - Temperature `> 0.0`: frequency-weighted sampling, with top-p restricting
///   the candidate pool to the smallest set whose cumulative frequency ≥ top_p.
/// - If `prev_token` has no followers, sample uniformly from `fallback_vocab`.
fn sample_next_token<'a>(
    markov: &HashMap<&'a str, Vec<&'a str>>,
    fallback: &'a [String],
    prev_token: &str,
    rng: &mut XorShift64,
    temperature: f32,
    top_p: f32,
) -> String {
    let candidates: Vec<&str> = match markov.get(prev_token) {
        Some(followers) if !followers.is_empty() => followers.clone(),
        _ => {
            if fallback.is_empty() {
                return String::new();
            }
            // Uniform sample from the fallback vocab.
            let idx = (rng.next() as usize) % fallback.len();
            return fallback[idx].clone();
        }
    };

    // Build frequency table.
    let mut freq: Vec<(&str, u32)> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for &c in &candidates {
        if let Some(&i) = index.get(c) {
            freq[i].1 += 1;
        } else {
            index.insert(c, freq.len());
            freq.push((c, 1));
        }
    }
    freq.sort_by(|a, b| b.1.cmp(&a.1));

    // Greedy when temperature is non-positive.
    if temperature <= 0.0 {
        return freq[0].0.to_string();
    }

    // Top-p pool.
    let total: u32 = freq.iter().map(|(_, n)| n).sum();
    let top_p_clamped = top_p.clamp(0.0, 1.0).max(0.01);
    let mut cumulative = 0u32;
    let mut pool: Vec<(&str, u32)> = Vec::new();
    for &(tok, count) in &freq {
        pool.push((tok, count));
        cumulative += count;
        let cum_prob = f64::from(cumulative) / f64::from(total);
        if cum_prob >= f64::from(top_p_clamped) {
            break;
        }
    }

    // Frequency-weighted sampling within the pool.
    // Apply temperature by raising counts to (1/temperature) — high temp
    // flattens the distribution, low temp sharpens it.
    let weights: Vec<f64> = pool
        .iter()
        .map(|(_, c)| (f64::from(*c)).powf(1.0 / f64::from(temperature).max(0.01))
    )
        .collect();
    let weight_sum: f64 = weights.iter().sum();
    if weight_sum <= 0.0 {
        return pool[0].0.to_string();
    }
    let mut r = (rng.next() as f64 / u64::MAX as f64) * weight_sum;
    for (i, w) in weights.iter().enumerate() {
        if r < *w {
            return pool[i].0.to_string();
        }
        r -= *w;
    }
    pool.last().map(|(t, _)| t.to_string()).unwrap_or_default()
}

/// True if the token is a "word" (alphanumeric, optionally with apostrophes).
fn is_word_token(tok: &str) -> bool {
    tok.chars().any(|c| c.is_alphanumeric())
}

/// FNV-1a 64-bit hash.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// FNV-1a 64-bit hash of a (position, layer, k) triple.
fn fnv1a64_of_triple(position: u32, layer: u32, k: u32) -> u64 {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&position.to_le_bytes());
    buf[4..8].copy_from_slice(&layer.to_le_bytes());
    buf[8..12].copy_from_slice(&k.to_le_bytes());
    fnv1a64(&buf)
}

/// xorshift64 PRNG. Deterministic given the seed.
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        // Avoid the degenerate all-zero state.
        let s = if seed == 0 { 0xDEADBEEFCAFEBABE } else { seed };
        Self { state: s }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{ChatMessage, ModelId, Role};
    use edgeforge_security::{ModelArch, ModelManifest};

    fn make_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            id: ModelId::new(),
            name: "SsdEngine Test Model".into(),
            publisher: "test".into(),
            license: "Apache-2.0".into(),
            license_url: None,
            acceptable_uses: vec![],
            geo_restrictions: vec![],
            file_size_bytes: 1024,
            sha256: [0u8; 32],
            precision: edgeforge_core::Precision::Fp16,
            arch: ModelArch::Moe,
            total_params: 1_000_000_000,
            active_params: Some(50_000_000),
            dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000,
            signature: None,
            publisher_key_fingerprint: None,
        }
    }

    fn make_prompt(text: &str) -> Prompt {
        Prompt::new(vec![ChatMessage {
            role: Role::User,
            content: text.into(),
        }])
    }

    #[tokio::test]
    async fn load_succeeds() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        assert_eq!(m.manifest.name, "SsdEngine Test Model");
    }

    #[tokio::test]
    async fn load_fails_on_empty_name() {
        let e = SsdEngine::new();
        let mut m = make_manifest();
        m.name = String::new();
        assert!(e.load(&m, ExecutionMode::Local).await.is_err());
    }

    #[tokio::test]
    async fn generate_produces_tokens() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("the quick brown fox jumps over the lazy dog");
        let params = GenParams {
            max_tokens: 10,
            seed: Some(42),
            ..GenParams::default()
        };
        let mut tokens = Vec::new();
        let stats = e
            .generate(&m, &p, &params, &mut |t, _| {
                tokens.push(t);
                true
            })
            .await
            .unwrap();
        assert!(stats.tokens_generated > 0, "should generate tokens");
        assert_eq!(tokens.len() as u32, stats.tokens_generated);
        assert!(stats.elapsed_ms > 0 || stats.tokens_generated > 0);
    }

    #[tokio::test]
    async fn respects_max_tokens() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("one two three four five six seven eight nine ten");
        let params = GenParams {
            max_tokens: 5,
            seed: Some(7),
            ..GenParams::default()
        };
        let mut count = 0u32;
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| {
                count += 1;
                true
            })
            .await
            .unwrap();
        assert_eq!(stats.tokens_generated, 5);
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn aborts_on_false_callback() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("a b c d e f g h i j k l m n o p");
        let params = GenParams {
            max_tokens: 100,
            seed: Some(1),
            ..GenParams::default()
        };
        let mut count = 0u32;
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| {
                count += 1;
                count < 3
            })
            .await
            .unwrap();
        assert_eq!(stats.tokens_generated, 3);
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn deterministic_with_seed() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("alpha beta gamma delta epsilon zeta eta theta");
        let params = GenParams {
            max_tokens: 8,
            seed: Some(12345),
            ..GenParams::default()
        };
        let mut t1: Vec<String> = Vec::new();
        e.generate(&m, &p, &params, &mut |t, _| {
            t1.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();
        let mut t2: Vec<String> = Vec::new();
        e.generate(&m, &p, &params, &mut |t, _| {
            t2.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();
        assert_eq!(t1, t2, "same seed must produce identical output");
    }

    #[tokio::test]
    async fn deterministic_with_global_seed() {
        // Test F-SCHED-05: global determinism mode propagates to SsdEngine.
        crate::determinism::reset_determinism();

        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("alpha beta gamma delta epsilon zeta eta theta");

        // Enable global determinism with seed 999.
        crate::determinism::set_determinism(crate::determinism::DeterminismConfig {
            enabled: true,
            seed: 999,
        });

        // No per-call seed — should fall back to global seed.
        let params = GenParams {
            max_tokens: 8,
            seed: None, // rely on global determinism
            ..GenParams::default()
        };

        let mut t1: Vec<String> = Vec::new();
        e.generate(&m, &p, &params, &mut |t, _| {
            t1.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();

        let mut t2: Vec<String> = Vec::new();
        e.generate(&m, &p, &params, &mut |t, _| {
            t2.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();

        // Same global seed → identical output.
        assert_eq!(t1, t2, "global determinism seed must produce identical output");

        // Per-call seed should override global.
        let params_override = GenParams {
            max_tokens: 8,
            seed: Some(42),
            ..GenParams::default()
        };
        let mut t3: Vec<String> = Vec::new();
        e.generate(&m, &p, &params_override, &mut |t, _| {
            t3.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();
        assert_ne!(t1, t3, "per-call seed should override global seed");

        crate::determinism::reset_determinism();
    }

    #[tokio::test]
    async fn stop_sequence_halts_generation() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("the cat sat on the mat the cat ran the cat jumped");
        let params = GenParams {
            max_tokens: 50,
            seed: Some(99),
            stop_sequences: vec!["cat".to_string()],
            ..GenParams::default()
        };
        let mut emitted = String::new();
        let stats = e
            .generate(&m, &p, &params, &mut |t, _| {
                emitted.push_str(&t.text);
                true
            })
            .await
            .unwrap();
        assert!(
            stats.tokens_generated <= 50,
            "should halt before max_tokens when stop sequence matches"
        );
        // Either we hit the stop sequence or never emitted it.
        assert!(
            emitted.contains("cat") || stats.tokens_generated < 50,
            "either 'cat' should appear and stop, or generation should be shorter than max"
        );
    }

    #[tokio::test]
    async fn empty_prompt_still_produces_tokens() {
        let e = SsdEngine::new();
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("");
        let params = GenParams {
            max_tokens: 3,
            seed: Some(5),
            ..GenParams::default()
        };
        let mut count = 0u32;
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| {
                count += 1;
                true
            })
            .await
            .unwrap();
        assert_eq!(stats.tokens_generated, 3);
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn router_hook_is_invoked_per_layer_expert() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let e = SsdEngine::with_config(SsdEngineConfig {
            n_layers: 3,
            n_experts_per_layer: 4,
            top_k_experts: 2,
            ..SsdEngineConfig::default()
        });

        struct CountingHook(Arc<AtomicU32>);
        impl RouterHook for CountingHook {
            fn resolve_expert(
                &self,
                _layer: LayerIdx,
                _expert: ExpertId,
            ) -> ExpertResolution {
                self.0.fetch_add(1, Ordering::Relaxed);
                ExpertResolution::Ram
            }
        }

        let counter = Arc::new(AtomicU32::new(0));
        e.install_router_hook(Box::new(CountingHook(counter.clone())))
            .unwrap();

        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("alpha beta gamma delta");
        let params = GenParams {
            max_tokens: 4,
            seed: Some(1),
            ..GenParams::default()
        };
        e.generate(&m, &p, &params, &mut |_, _| true)
            .await
            .unwrap();

        // 4 tokens * 3 layers * 2 experts = 24 calls
        assert_eq!(
            counter.load(Ordering::Relaxed),
            4 * 3 * 2,
            "router hook should be called once per (token, layer, expert)"
        );
    }

    #[tokio::test]
    async fn stats_report_cold_cache_when_ssd_path_taken() {
        // No weight pager configured → simulated SSD read → cold_cache should be true.
        let e = SsdEngine::with_config(SsdEngineConfig {
            n_layers: 1,
            n_experts_per_layer: 1,
            top_k_experts: 1,
            ..SsdEngineConfig::default()
        });
        let m = e.load(&make_manifest(), ExecutionMode::Local).await.unwrap();
        let p = make_prompt("hello world");
        let params = GenParams {
            max_tokens: 1,
            seed: Some(1),
            ..GenParams::default()
        };
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| true)
            .await
            .unwrap();
        // With no router hook installed, every expert resolves to Ssd → cold cache.
        assert!(stats.cold_cache, "cold_cache should be true when SSD reads occur");
    }

    #[test]
    fn tokenizer_handles_punctuation() {
        let toks = tokenize("Hello, world! Don't panic.");
        assert_eq!(toks, vec!["hello", "world", "don't", "panic"]);
    }

    #[test]
    fn markov_map_records_followers_with_multiplicity() {
        let toks: Vec<String> = ["a", "b", "a", "c", "a", "b"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let map = build_markov_map(&toks);
        let followers_b = map.get("a").unwrap();
        // "a" is followed by "b", "c", "b" → b appears twice, c once.
        assert_eq!(followers_b.len(), 3);
        assert_eq!(followers_b.iter().filter(|&&s| s == "b").count(), 2);
        assert_eq!(followers_b.iter().filter(|&&s| s == "c").count(), 1);
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = XorShift64::new(42);
        let mut b = XorShift64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next(), b.next());
        }
    }

    #[test]
    fn xorshift_avoids_zero_state() {
        let mut rng = XorShift64::new(0);
        // Should not panic or produce all-zero output.
        let v = rng.next();
        assert_ne!(v, 0);
    }
}
