// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;
macro_rules! id_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        #[repr(transparent)]
        pub struct $name(pub Ulid);
        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Ulid::new())
            }
            pub fn parse(s: &str) -> Result<Self, ulid::DecodeError> {
                Ulid::from_string(s).map(Self)
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}
id_newtype!(ModelId, "Model identifier.");
id_newtype!(JobId, "Job identifier.");
id_newtype!(ConversationId, "Conversation identifier.");
id_newtype!(MessageId, "Message identifier.");
id_newtype!(NodeId, "Node identifier.");
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)]
#[repr(transparent)]
pub struct LayerIdx(pub u32);
impl LayerIdx {
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ExpertId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ExpertSource {
    RamCache,
    LocalSsd,
    RemotePeer(NodeId),
}
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    pub id: u32,
}
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub messages: Vec<crate::config::ChatMessage>,
}
impl Prompt {
    #[must_use]
    pub fn new(messages: Vec<crate::config::ChatMessage>) -> Self {
        Self { messages }
    }
    #[must_use]
    pub fn render_flat(&self) -> String {
        let mut out = String::with_capacity(256);
        for m in &self.messages {
            out.push_str(m.role.as_str());
            out.push_str(": ");
            out.push_str(&m.content);
            out.push('\n');
        }
        out
    }
}
