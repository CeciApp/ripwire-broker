//! Sprint 0 (S4.0b): prints the `prompts/v1` requests for a fixed synthetic corpus, one JSON
//! per line, so a live recording can send them without a network crate in the build:
//!
//! ```text
//! cargo run -q --example jev_record > requests.jsonl
//! # each line: curl -sS https://api.typesafe.ai/v1/systemone \
//! #   -H "Authorization: Bearer $RIPWIRE_BROKER_JEV_API_KEY" \
//! #   -H 'Content-Type: application/json' --data-binary @line.json
//! ```
//!
//! The corpus (`tests/common/jev_corpus.rs`) is invented; no workspace content is ever read.
#[path = "../tests/common/jev_corpus.rs"]
mod jev_corpus;

fn main() {
    for req in jev_corpus::requests() {
        println!("{}", serde_json::to_string(&req).unwrap());
    }
}
