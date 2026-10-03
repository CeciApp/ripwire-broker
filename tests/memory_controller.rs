//! Seam: the memory controller (PRD jev-mem §7, §8): what it asks the classifier and what it
//! does with the answers. A classifier stand-in, no network.
use ripwire_broker::memory::prompts::{self, Stage};
use ripwire_broker::online::request::JevQuestionType;

#[test]
fn every_stage_names_its_state_fields_and_carries_the_untrusted_guidance() {
    assert_eq!(prompts::VERSION, "memory-prompts/v1");
    assert_ne!(prompts::VERSION, ripwire_broker::notes::PROMPT_VERSION);

    let expected: [(Stage, &[&str], usize, usize); 8] = [
        (
            Stage::Typing,
            &["episodic", "semantic", "procedural", "preference"],
            4,
            0,
        ),
        (
            Stage::Relations,
            &["semantic", "caused_by", "causes", "same_episode"],
            4,
            0,
        ),
        (Stage::Alias, &["alias"], 1, 0),
        (Stage::ImplicitTime, &["time"], 0, 1),
        (
            Stage::Routing,
            &[
                "semantic",
                "temporal",
                "causal",
                "entity",
                "multi_hop_need",
                "recency_importance",
            ],
            6,
            0,
        ),
        (
            Stage::Scoring,
            &[
                "relevance",
                "new_information",
                "relation_usefulness",
                "supports_current_evidence",
            ],
            4,
            0,
        ),
        (
            Stage::Stopping,
            &[
                "evidence_sufficient",
                "continue_useful",
                "missing_evidence",
                "contradiction",
            ],
            4,
            0,
        ),
        (
            Stage::Consolidation,
            &[
                "redundancy",
                "contradiction",
                "obsolescence",
                "link_usefulness",
                "representation",
            ],
            4,
            1,
        ),
    ];
    for (stage, names, nouls, choices) in expected {
        let questions = prompts::questions(stage, 2);
        let got: Vec<&str> = questions.iter().map(|(n, _)| *n).collect();
        assert_eq!(got, names, "{stage:?}");
        let count = |k| questions.iter().filter(|(_, q)| q.kind == k).count();
        assert_eq!(
            (count(JevQuestionType::Noul), count(JevQuestionType::Choice)),
            (nouls, choices),
            "{stage:?}"
        );

        let fields = prompts::state_fields(stage);
        assert!(!fields.is_empty(), "{stage:?}");
        for field in fields {
            assert!(
                questions
                    .iter()
                    .any(|(_, q)| q.instructions.contains(field)),
                "{stage:?}: no question names `{field}`"
            );
        }
        for (name, q) in &questions {
            let text = &q.instructions;
            assert!(
                text.contains("true:") && text.contains("false:"),
                "{stage:?}/{name}: explicit criteria"
            );
            assert!(text.contains("untrusted"), "{stage:?}/{name}: the guidance");
            for (other, _) in questions.iter().filter(|(o, _)| o != name) {
                assert!(
                    !text.contains(&format!("answer to {other}"))
                        && !text.contains("other question"),
                    "{stage:?}/{name} leans on {other}"
                );
            }
        }
    }
    // A per-candidate stage points at the candidate it is about.
    let relations = prompts::questions(Stage::Relations, 3);
    assert!(
        relations
            .iter()
            .all(|(_, q)| q.instructions.contains("candidates[3]"))
    );
    let time = &prompts::questions(Stage::ImplicitTime, 0)[0].1;
    let options: Vec<&str> = time
        .criteria
        .as_ref()
        .unwrap()
        .0
        .iter()
        .map(|(o, _)| o.as_str())
        .collect();
    assert_eq!(
        options,
        [
            "before",
            "after",
            "during",
            "contains",
            "overlaps",
            "same_time",
            "unknown"
        ],
        "direction new_memory → candidate"
    );
    let representation = &prompts::questions(Stage::Consolidation, 0)[4].1;
    let options: Vec<&str> = representation
        .criteria
        .as_ref()
        .unwrap()
        .0
        .iter()
        .map(|(o, _)| o.as_str())
        .collect();
    assert_eq!(options, ["keep_separate", "merge", "promote", "uncertain"]);
}
