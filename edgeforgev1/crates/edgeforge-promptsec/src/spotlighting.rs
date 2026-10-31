// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
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
pub enum SpotlightingTechnique {
    DataMarking,
    DataEncapsulation,
    DataTrimming,
    All,
}
#[derive(Clone, Debug)]
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
    pub fn new(config: SpotlightingConfig) -> Self {
        Self { config }
    }
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
