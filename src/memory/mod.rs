//! Persistent per-workspace memory behind `--memory` (PRD `docs/jev-mem-prd.md`). The domain
//! and the store compile in the default build, without a network stack (CA-10); the Jev
//! controller lives behind the `online` feature.

pub mod admission;
pub mod identity;
pub mod model;
pub mod store;
pub mod time;
