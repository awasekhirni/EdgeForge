//! # Physical-Unit Newtypes
//!
//! ## Purpose
//! Wraps raw numeric types to prevent unit confusion (e.g. comparing
//! bytes-per-second to bytes-total). Each newtype supports arithmetic operators.
//!
//! ## What This File Does
//! - `Bytes(u64)` — byte count with `from_gib()`, `as_gib_f64()`, Add, Sub
//! - `TokPerSec(f32)` — inference throughput metric
//! - `Celsius(f32)` — SSD temperature with THROTTLE (70°C) and CRITICAL (80°C)
//!
//! ## Dependencies
//! - `serde` — Serialize/Deserialize
//!
//! ## Output
//! Type-safe unit wrappers that prevent mixing units in arithmetic.
//!
//! ## Key Constants
//! - `Celsius::THROTTLE = 70.0` — prefetch depth reduced
//! - `Celsius::CRITICAL = 80.0` — inference paused

use serde::{Deserialize, Serialize}; use std::fmt; use std::ops::{Add, Sub};
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default, Debug)]
#[serde(transparent)] #[repr(transparent)]
/// `Bytes` — see module documentation for purpose and usage.
pub struct Bytes(pub u64);
impl Bytes { pub const fn from_gib(g: u64) -> Self { Self(g * 1024 * 1024 * 1024) } }
impl Add for Bytes { type Output = Self; fn add(self, r: Self) -> Self { Self(self.0 + r.0) } }
impl Sub for Bytes { type Output = Self; fn sub(self, r: Self) -> Self { Self(self.0.saturating_sub(r.0)) } }
impl fmt::Display for Bytes { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{} B", self.0) } }
#[derive(Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default, Debug)]
#[serde(transparent)] #[repr(transparent)]
/// `TokPerSec` — see module documentation for purpose and usage.
pub struct TokPerSec(pub f32);
impl fmt::Display for TokPerSec { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:.2} tok/s", self.0) } }
#[derive(Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default, Debug)]
#[serde(transparent)] #[repr(transparent)]
/// `Celsius` — see module documentation for purpose and usage.
pub struct Celsius(pub f32);
impl Celsius { pub const THROTTLE: Self = Self(70.0); pub const CRITICAL: Self = Self(80.0); }
impl fmt::Display for Celsius { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:.1}°C", self.0) } }
/// Bytes — see module documentation for details.
pub fn bytes(n: impl Into<u64>) -> Bytes { Bytes(n.into()) }
pub const fn celsius(c: f32) -> Celsius { Celsius(c) }
pub const fn tok_per_sec(t: f32) -> TokPerSec { TokPerSec(t) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_from_gib() {
        let b = Bytes::from_gib(1);
        assert_eq!(b.0, 1024 * 1024 * 1024);
        let b2 = Bytes::from_gib(2);
        assert_eq!(b2.0, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_bytes_add() {
        let a = Bytes(100);
        let b = Bytes(200);
        assert_eq!((a + b).0, 300);
    }

    #[test]
    fn test_bytes_sub() {
        let a = Bytes(300);
        let b = Bytes(100);
        assert_eq!((a - b).0, 200);
    }

    #[test]
    fn test_bytes_sub_saturating() {
        let a = Bytes(50);
        let b = Bytes(100);
        assert_eq!((a - b).0, 0, "subtraction should saturate at 0");
    }

    #[test]
    fn test_bytes_display() {
        let b = Bytes(1024);
        assert_eq!(b.to_string(), "1024 B");
    }

    #[test]
    fn test_bytes_equality_and_ordering() {
        assert_eq!(Bytes(100), Bytes(100));
        assert!(Bytes(100) < Bytes(200));
        assert!(Bytes(200) > Bytes(100));
    }

    #[test]
    fn test_tok_per_sec_display() {
        let t = TokPerSec(42.5);
        let s = t.to_string();
        assert!(s.contains("42.50"));
        assert!(s.contains("tok/s"));
    }

    #[test]
    fn test_tok_per_sec_default() {
        assert_eq!(TokPerSec::default().0, 0.0);
    }

    #[test]
    fn test_celsius_throttle_constant() {
        assert_eq!(Celsius::THROTTLE.0, 70.0);
    }

    #[test]
    fn test_celsius_critical_constant() {
        assert_eq!(Celsius::CRITICAL.0, 80.0);
    }

    #[test]
    fn test_celsius_display() {
        let c = Celsius(45.5);
        let s = c.to_string();
        assert!(s.contains("45.5"));
        assert!(s.contains("°C"));
    }

    #[test]
    fn test_celsius_comparison() {
        assert!(Celsius(50.0) < Celsius::THROTTLE);
        assert!(Celsius(75.0) > Celsius::THROTTLE);
        assert!(Celsius(75.0) < Celsius::CRITICAL);
        assert!(Celsius(85.0) > Celsius::CRITICAL);
    }

    #[test]
    fn test_bytes_helper_function() {
        let b = bytes(42u64);
        assert_eq!(b.0, 42);
    }

    #[test]
    fn test_celsius_helper_function() {
        let c = celsius(42.0);
        assert_eq!(c.0, 42.0);
    }

    #[test]
    fn test_tok_per_sec_helper_function() {
        let t = tok_per_sec(100.0);
        assert_eq!(t.0, 100.0);
    }
}
