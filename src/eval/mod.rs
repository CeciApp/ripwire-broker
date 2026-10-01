//! The A/B instrument (PRD §16.2–16.4, §17, §23.15; D-116): a corpus of tasks with a reference
//! patch, run by a real agent in each arm, reduced to counts and scored against the PRD's bars.
//! It drives the agent from outside and never runs inside the broker: nothing here is reachable
//! from `serve`, the hooks or any other command of `ripwire-broker`. The binary is `ripwire-eval`.

pub mod arm;
pub mod corpus;
pub mod report;
pub mod runner;
pub mod score;
pub mod transcript;
