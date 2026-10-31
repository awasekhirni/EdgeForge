// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default, Debug,
)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Bytes(pub u64);
impl Bytes {
    pub const fn from_gib(g: u64) -> Self {
        Self(g * 1024 * 1024 * 1024)
    }
}
impl Add for Bytes {
    type Output = Self;
    fn add(self, r: Self) -> Self {
        Self(self.0 + r.0)
    }
}
impl Sub for Bytes {
    type Output = Self;
    fn sub(self, r: Self) -> Self {
        Self(self.0.saturating_sub(r.0))
    }
}
impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} B", self.0)
    }
}
#[derive(Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default, Debug)]
#[serde(transparent)]
#[repr(transparent)]
pub struct TokPerSec(pub f32);
impl fmt::Display for TokPerSec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2} tok/s", self.0)
    }
}
#[derive(Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default, Debug)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Celsius(pub f32);
impl Celsius {
    pub const THROTTLE: Self = Self(70.0);
    pub const CRITICAL: Self = Self(80.0);
}
impl fmt::Display for Celsius {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}°C", self.0)
    }
}
pub fn bytes(n: impl Into<u64>) -> Bytes {
    Bytes(n.into())
}
pub const fn celsius(c: f32) -> Celsius {
    Celsius(c)
}
pub const fn tok_per_sec(t: f32) -> TokPerSec {
    TokPerSec(t)
}
