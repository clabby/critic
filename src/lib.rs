//! critic library crate.

pub mod app;
pub mod config;
pub mod domain;
pub mod github;
#[cfg(feature = "harness")]
pub mod harness;
pub mod render;
pub mod search;
pub mod ui;
