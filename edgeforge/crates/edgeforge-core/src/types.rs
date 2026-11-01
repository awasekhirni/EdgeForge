//! # Domain Newtypes
//!
//! ## Purpose
//! Type-safe identifiers and domain types that prevent argument-order bugs.
//! Each ID type wraps a `ULID` for sortability and uniqueness.
//!
//! ## What This File Does
//! - Defines `ModelId`, `JobId`, `ConversationId`, `MessageId`, `NodeId` via macro
//! - Defines `LayerIdx` (zero-based transformer layer index)
//! - Defines `ExpertId` (expert within an MoE layer)
//! - Defines `ExpertSource` (RAM cache / local SSD / remote peer)
//! - Defines `Token` (generated text + vocab ID)
//! - Defines `Prompt` (sequence of chat messages)
//!
//! ## Dependencies
//! - `serde` — Serialize/Deserialize for all types
//! - `ulid::Ulid` — 128-bit sortable identifier
//!
//! ## Output
//! Type-safe newtypes that the compiler uses to catch argument-order errors.
//!
//! ## Key Functions
//! - `ModelId::new()` — generates a fresh ULID-based ID
//! - `ModelId::parse(s)` — parses a 26-char ULID string
//! - `Prompt::render_flat()` — renders prompt as `role: content\n` per message

use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;
macro_rules! id_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)] #[repr(transparent)]
        pub struct $name(pub Ulid);
        impl $name {
            #[must_use] pub fn new() -> Self { Self(Ulid::new()) }
            /// Parse — see module documentation for details.
            pub fn parse(s: &str) -> Result<Self, ulid::DecodeError> { Ulid::from_string(s).map(Self) }
        }
        impl Default for $name { fn default() -> Self { Self::new() } }
        impl fmt::Display for $name { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) } }
        impl fmt::Debug for $name { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, concat!(stringify!($name), "({})"), self.0) } }
    };
}
id_newtype!(ModelId, "Model identifier.");
id_newtype!(JobId, "Job identifier.");
id_newtype!(ConversationId, "Conversation identifier.");
id_newtype!(MessageId, "Message identifier.");
id_newtype!(NodeId, "Node identifier.");
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)] #[repr(transparent)]
/// `LayerIdx` — see module documentation for purpose and usage.
pub struct LayerIdx(pub u32);
impl LayerIdx { #[must_use] pub const fn next(self) -> Self { Self(self.0 + 1) } }
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)] #[repr(transparent)]
/// `ExpertId` — see module documentation for purpose and usage.
pub struct ExpertId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
/// `ExpertSource` enumeration — see module documentation for variants.
pub enum ExpertSource { RamCache, LocalSsd, RemotePeer(NodeId) }
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
/// `Token` — see module documentation for purpose and usage.
pub struct Token { pub text: String, pub id: u32 }
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
/// `Prompt` — see module documentation for purpose and usage.
pub struct Prompt { pub messages: Vec<crate::config::ChatMessage> }
impl Prompt {
    #[must_use] pub fn new(messages: Vec<crate::config::ChatMessage>) -> Self { Self { messages } }
    #[must_use] pub fn render_flat(&self) -> String {
        let mut out = String::with_capacity(256);
        for m in &self.messages { out.push_str(m.role.as_str()); out.push_str(": "); out.push_str(&m.content); out.push('\n'); }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ChatMessage, Role};

    #[test]
    fn test_model_id_new_is_unique() {
        let a = ModelId::new();
        let b = ModelId::new();
        assert_ne!(a, b, "two new IDs must differ");
    }

    #[test]
    fn test_model_id_parse_and_display_roundtrip() {
        let id = ModelId::new();
        let s = id.to_string();
        let parsed = ModelId::parse(&s).unwrap();
        assert_eq!(id, parsed, "parse(display(id)) must roundtrip");
    }

    #[test]
    fn test_model_id_parse_invalid() {
        assert!(ModelId::parse("not-a-ulid").is_err());
        assert!(ModelId::parse("").is_err());
    }

    #[test]
    fn test_job_id_parse_and_display() {
        let id = JobId::new();
        let s = id.to_string();
        let parsed = JobId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_node_id_debug_format() {
        let id = NodeId::new();
        let debug = format!("{id:?}");
        assert!(debug.starts_with("NodeId("));
    }

    #[test]
    fn test_layer_idx_next() {
        let l0 = LayerIdx(0);
        assert_eq!(l0.next(), LayerIdx(1));
        assert_eq!(LayerIdx(41).next(), LayerIdx(42));
    }

    #[test]
    fn test_layer_idx_equality() {
        assert_eq!(LayerIdx(5), LayerIdx(5));
        assert_ne!(LayerIdx(5), LayerIdx(6));
    }

    #[test]
    fn test_expert_id_equality() {
        assert_eq!(ExpertId(0), ExpertId(0));
        assert_ne!(ExpertId(0), ExpertId(1));
    }

    #[test]
    fn test_expert_source_variants() {
        let ram = ExpertSource::RamCache;
        let ssd = ExpertSource::LocalSsd;
        let peer = ExpertSource::RemotePeer(NodeId::new());
        assert!(matches!(ram, ExpertSource::RamCache));
        assert!(matches!(ssd, ExpertSource::LocalSsd));
        assert!(matches!(peer, ExpertSource::RemotePeer(_)));
    }

    #[test]
    fn test_token_fields() {
        let t = Token { text: "hello".into(), id: 42 };
        assert_eq!(t.text, "hello");
        assert_eq!(t.id, 42);
    }

    #[test]
    fn test_prompt_new() {
        let p = Prompt::new(vec![ChatMessage { role: Role::User, content: "hi".into() }]);
        assert_eq!(p.messages.len(), 1);
    }

    #[test]
    fn test_prompt_render_flat_single_message() {
        let p = Prompt::new(vec![ChatMessage { role: Role::User, content: "hello world".into() }]);
        let flat = p.render_flat();
        assert!(flat.contains("user: hello world"));
        assert!(flat.ends_with('\n'));
    }

    #[test]
    fn test_prompt_render_flat_multiple_messages() {
        let p = Prompt::new(vec![
            ChatMessage { role: Role::System, content: "You are helpful".into() },
            ChatMessage { role: Role::User, content: "Hi".into() },
        ]);
        let flat = p.render_flat();
        assert!(flat.contains("system: You are helpful"));
        assert!(flat.contains("user: Hi"));
        assert_eq!(flat.matches('\n').count(), 2);
    }

    #[test]
    fn test_prompt_render_flat_empty() {
        let p = Prompt::new(vec![]);
        assert_eq!(p.render_flat(), "");
    }

    #[test]
    fn test_id_default() {
        let a = ModelId::default();
        let b = ModelId::default();
        assert_ne!(a, b, "default must generate unique IDs");
    }

    #[test]
    fn test_id_ordering() {
        let a = ModelId::new();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = ModelId::new();
        // ULIDs are sortable by time — b should be >= a.
        assert!(b >= a, "later ULID should be >= earlier ULID");
    }
}
