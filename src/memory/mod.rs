//! Persistent per-workspace memory behind `--memory` (PRD `docs/jev-mem-prd.md`). The domain
//! and the store compile in the default build, without a network stack (CA-10); the Jev
//! controller lives behind the `online` feature.

pub mod admission;
pub mod command;
pub mod consolidate;
pub mod controller;
pub mod identity;
pub mod index;
pub mod metrics;
pub mod model;
pub mod prompts;
pub mod publish;
pub mod queue;
pub mod recall;
pub mod retrieve;
pub mod runtime;
pub mod store;
pub mod time;
pub mod wire;
