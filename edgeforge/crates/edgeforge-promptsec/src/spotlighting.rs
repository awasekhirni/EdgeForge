// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Spotlighting (ADR-021)
//!
//! ## Purpose
//! Three techniques that visually and structurally separate untrusted external
//! content from trusted instructions. The worker LLM is trained to treat
//! spotlighted content as data, not commands.
//!
//! ## What This File Does
//! - `SpotlightingConfig` — data_marking, data_encapsulation, data_trimming, max_content_chars
//! - `Spotlighter` — applies configured techniques to untrusted content
//! - `spotlight(untrusted)` — applies configured techniques
//! - `spotlight_all(untrusted)` — applies all 3 in canonical order: trim → mark → encapsulate
//! - Data marking: wraps in `[UNTRUSTED_BEGIN]...[UNTRUSTED_END]` + markdown fences
//! - Data encapsulation: base64-encodes so content cannot be parsed as instructions
//! - Data trimming: strips 12 injection patterns ("ignore previous", "you are now", etc.)
//!
//! ## Dependencies
//! - `once_cell` — lazy static regex patterns
//! - `regex` — injection pattern matching
//! - `base64` — data encapsulation encoding
//! - `serde` — config serialization
//!
//! ## Output
//! `String` — the spotlighted content (what the worker LLM sees, never the raw content).
//!
//! ## Function: Spotlighter::spotlight
//! - **Purpose:** Apply configured spotlighting techniques to untrusted content
//! - **Parameters:** `&str` — the untrusted content
//! - **Returns:** `String` — the spotlighted output
//! - **Processing order:** trim → truncate → mark → encapsulate (based on config)
//! - **Dependencies:** `self.trim()`, `self.mark()`, `self.encapsulate()`
//!
//! ## Trimming Denylist
//! Patterns removed (case-insensitive): "ignore previous instructions",
//! "ignore prior instructions", "you are now a/an", "system:", "assistant:",
//! "reveal your system prompt", "print your system prompt", and more.

use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `SpotlightingConfig` — see module documentation for purpose and usage.
pub struct SpotlightingConfig {
    pub data_marking: bool,
    pub data_encapsulation: bool,
    pub data_trimming: bool,
    pub max_content_chars: usize,
}
impl Default for SpotlightingConfig {
    fn default() -> Self {
        Self {
            data_marking: true,
            data_encapsulation: false,
            data_trimming: true,
            max_content_chars: 8192,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// `SpotlightingTechnique` enumeration — see module documentation for variants.
pub enum SpotlightingTechnique {
    DataMarking,
    DataEncapsulation,
    DataTrimming,
    All,
}
#[derive(Clone, Debug)]
/// `Spotlighter` — see module documentation for purpose and usage.
pub struct Spotlighter {
    pub config: SpotlightingConfig,
}
impl Default for Spotlighter {
    fn default() -> Self {
        Self {
            config: SpotlightingConfig::default(),
        }
    }
}
impl Spotlighter {
    /// Creates a new instance with default configuration.
    pub fn new(config: SpotlightingConfig) -> Self {
        Self { config }
    }
    /// Applies spotlighting techniques to untrusted content.
    pub fn spotlight(&self, untrusted: &str) -> String {
        let mut content = untrusted.to_string();
        if self.config.data_trimming {
            content = self.trim(&content);
        }
        if content.len() > self.config.max_content_chars {
            content.truncate(self.config.max_content_chars);
            content.push_str("\n[... truncated ...]");
        }
        if self.config.data_marking {
            content = self.mark(&content);
        }
        if self.config.data_encapsulation {
            content = self.encapsulate(&content);
        }
        content
    }
    /// Applies all spotlighting techniques in canonical order.
    pub fn spotlight_all(&self, untrusted: &str) -> String {
        let t = self.trim(untrusted);
        let m = self.mark(&t);
        self.encapsulate(&m)
    }
    fn trim(&self, content: &str) -> String {
        static PATS: once_cell::sync::Lazy<Vec<regex::Regex>> = once_cell::sync::Lazy::new(|| {
            vec![
                regex::Regex::new("(?i)ignore (all )?previous instructions").unwrap(),
                regex::Regex::new("(?i)ignore (all )?prior instructions").unwrap(),
                regex::Regex::new("(?i)you are now (a|an) ").unwrap(),
                regex::Regex::new("(?i)system:").unwrap(),
                regex::Regex::new("(?i)assistant:").unwrap(),
                regex::Regex::new("(?i)reveal (your|the) (system )?prompt").unwrap(),
            ]
        });
        let mut r = content.to_string();
        for re in PATS.iter() {
            r = re.replace_all(&r, "[REMOVED]").to_string();
        }
        r
    }
    fn mark(&self, content: &str) -> String {
        format!("[UNTRUSTED_BEGIN]\n```\n{content}\n```\n[UNTRUSTED_END]")
    }
    fn encapsulate(&self, content: &str) -> String {
        use base64::Engine;
        let e = base64::engine::general_purpose::STANDARD.encode(content);
        format!("[BASE64_BEGIN]\n{e}\n[BASE64_END]")
    }
}
