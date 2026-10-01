//! `ripwire-broker` as a library: the broker core and the modules the binary drives.
//!
//! Two lints are denied crate-wide rather than trusted to review. `unsafe_code` is
//! forbidden because this crate has none and should keep none. `print_stdout` and
//! `dbg_macro` are denied because in `serve` stdout carries the MCP protocol: one stray
//! `println!` in the library corrupts the session. `main.rs` is a separate crate root, so
//! the legitimate `println!` of the one-shot commands is unaffected (D-107).
#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::dbg_macro)]

pub mod broker;
mod budget;
pub mod cli;
mod dedup;
pub mod doctor;
/// The A/B instrument behind the `ripwire-eval` binary (D-116). Nothing in the broker calls it.
pub mod eval;
pub mod hook;
pub mod install;
pub mod local;
/// Public for the property tests that fuzz it over arbitrary input (D-111). It reads another
/// process's stdout and has a history of panics there, so the property has to reach it directly
/// rather than through a broker. Internal: no stability promise.
pub mod markup;
pub mod mcp;
pub mod metrics;
pub mod model;
mod normalize;
pub mod notes;
pub mod online;
mod router;
pub mod session;
pub mod state;
pub mod statusline;
pub mod statusline_state;
pub mod summarizer;
pub mod supervise;
pub mod upstream;
pub mod usage;
pub mod workspace;
