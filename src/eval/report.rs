//! The results of every run, reduced per arm and held against the PRD's bars (§17 and §23.15).
//! A bar is `insuficiente` until the corpus is big enough (≥ 30 tasks in ≥ 3 repositories) and
//! both arms it compares have valid runs: a small corpus is not a result, whatever it says.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// One run of one task in one arm: counts and scores only. The transcript, which holds the
/// agent's commands and the repository's code, stays in its own file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub task: String,
    pub repo: String,
    pub arm: String,
    pub repeat: u32,
    pub vocabulary_diverges: bool,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid: Option<String>,
    pub is_error: bool,
    pub tokens: u64,
    pub cost_usd: f64,
    pub duration_ms: u64,
    pub search: u64,
    pub read: u64,
    pub edit: u64,
    pub mcp: u64,
    pub other: u64,
    pub mcp_result_bytes: u64,
    pub first_edit_ms: Option<u64>,
    pub file_recall: f64,
    pub file_precision: Option<f64>,
    pub presented_recall: Option<f64>,
    pub first_correct_rank: Option<usize>,
    pub test_recall: f64,
    pub correct: Option<bool>,
}

impl RunRecord {
    pub fn exploratory(&self) -> u64 {
        self.search + self.read
    }

    /// The key a resumed run is matched on.
    pub fn key(&self) -> (String, String, u32) {
        (self.task.clone(), self.arm.clone(), self.repeat)
    }
}

/// `results.jsonl` in `out`; lines that do not parse are skipped.
pub fn load(out: &Path) -> Vec<RunRecord> {
    std::fs::read_to_string(out.join("results.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub const MIN_TASKS: usize = 30;
pub const MIN_REPOS: usize = 3;
/// Room for floating point at the exact threshold: 35% fewer is a pass, not a rounding error.
const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Pass,
    Fail,
    Insufficient,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "passa",
            Verdict::Fail => "falha",
            Verdict::Insufficient => "insuficiente",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bar {
    pub id: &'static str,
    pub claim: &'static str,
    /// The arm compared against, and its value.
    pub baseline: Option<f64>,
    pub value: Option<f64>,
    pub verdict: Verdict,
}

/// One arm's valid runs, reduced.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ArmStats {
    pub arm: String,
    pub runs: usize,
    pub valid: usize,
    pub tasks: usize,
    pub repos: usize,
    pub tokens: Option<f64>,
    pub exploratory: Option<f64>,
    pub mcp_result_bytes: Option<f64>,
    pub cost_usd: f64,
    /// Over the runs whose task has a `check`.
    pub completion: Option<f64>,
    pub file_recall: Option<f64>,
    pub presented_recall: Option<f64>,
    pub test_recall: Option<f64>,
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / n as f64)
}

fn valid<'a>(records: &'a [RunRecord], arm: &str) -> Vec<&'a RunRecord> {
    records.iter().filter(|r| r.arm == arm && r.valid).collect()
}

/// For a broker arm, presenting nothing is a recall of zero, not a missing value.
fn presented(r: &RunRecord) -> f64 {
    r.presented_recall.unwrap_or(0.0)
}

pub fn arm_stats(records: &[RunRecord], arm: &str) -> ArmStats {
    let v = valid(records, arm);
    let f = |g: fn(&RunRecord) -> f64| mean(v.iter().map(|r| g(r)));
    ArmStats {
        arm: arm.into(),
        runs: records.iter().filter(|r| r.arm == arm).count(),
        valid: v.len(),
        tasks: v.iter().map(|r| &r.task).collect::<BTreeSet<_>>().len(),
        repos: v.iter().map(|r| &r.repo).collect::<BTreeSet<_>>().len(),
        tokens: f(|r| r.tokens as f64),
        exploratory: f(|r| r.exploratory() as f64),
        mcp_result_bytes: f(|r| r.mcp_result_bytes as f64),
        cost_usd: v.iter().map(|r| r.cost_usd).sum(),
        completion: mean(
            v.iter()
                .filter_map(|r| r.correct)
                .map(|c| if c { 1.0 } else { 0.0 }),
        ),
        file_recall: f(|r| r.file_recall),
        presented_recall: mean(v.iter().filter_map(|r| r.presented_recall)),
        test_recall: f(|r| r.test_recall),
    }
}

/// Whether two arms were compared over a corpus big enough to mean anything.
fn sufficient(records: &[RunRecord], a: &str, b: &str) -> bool {
    let tasks = |arm| {
        valid(records, arm)
            .into_iter()
            .map(|r| (r.task.clone(), r.repo.clone()))
            .collect::<BTreeSet<_>>()
    };
    let both: BTreeSet<_> = tasks(a).intersection(&tasks(b)).cloned().collect();
    let repos = both.iter().map(|(_, r)| r).collect::<BTreeSet<_>>().len();
    both.len() >= MIN_TASKS && repos >= MIN_REPOS
}

/// `1 - value / baseline`: how much less the arm spent.
fn reduction(baseline: Option<f64>, value: Option<f64>) -> Option<f64> {
    match (baseline, value) {
        (Some(b), Some(v)) if b > 0.0 => Some(1.0 - v / b),
        _ => None,
    }
}

fn judge(enough: bool, holds: Option<bool>) -> Verdict {
    match (enough, holds) {
        (true, Some(true)) => Verdict::Pass,
        (true, Some(false)) => Verdict::Fail,
        _ => Verdict::Insufficient,
    }
}

pub fn bars(records: &[RunRecord]) -> Vec<Bar> {
    let none = arm_stats(records, "none");
    let broker = arm_stats(records, "broker");
    let online = arm_stats(records, "broker-online");
    let offline_ok = sufficient(records, "none", "broker");
    let online_ok = sufficient(records, "broker", "broker-online");

    let tokens = reduction(none.tokens, broker.tokens);
    let explore = reduction(none.exploratory, broker.exploratory);
    let explore_online = reduction(broker.exploratory, online.exploratory);

    let divergent = |arm: &str| {
        mean(
            valid(records, arm)
                .into_iter()
                .filter(|r| r.vocabulary_diverges)
                .map(presented),
        )
    };
    let (div_off, div_on) = (divergent("broker"), divergent("broker-online"));
    let gain = div_off.zip(div_on).map(|(off, on)| on - off);

    let at_least = |v: Option<f64>, min: f64| v.map(|v| v >= min - EPS);
    let not_below = |base: Option<f64>, v: Option<f64>| base.zip(v).map(|(b, v)| v >= b - EPS);

    vec![
        Bar {
            id: "17.1",
            claim: "broker × none: ≥ 35% menos tokens do agente",
            baseline: none.tokens,
            value: broker.tokens,
            verdict: judge(offline_ok, at_least(tokens, 0.35)),
        },
        Bar {
            id: "17.2",
            claim: "broker × none: ≥ 30% menos chamadas exploratórias (busca + leitura)",
            baseline: none.exploratory,
            value: broker.exploratory,
            verdict: judge(offline_ok, at_least(explore, 0.30)),
        },
        Bar {
            id: "17.3",
            claim: "broker × none: mantém ou melhora a taxa de conclusão correta",
            baseline: none.completion,
            value: broker.completion,
            verdict: judge(offline_ok, not_below(none.completion, broker.completion)),
        },
        Bar {
            id: "17.4",
            claim: "broker × none: melhora o recall dos arquivos do patch de referência",
            baseline: none.file_recall,
            value: broker.file_recall,
            verdict: judge(
                offline_ok,
                none.file_recall
                    .zip(broker.file_recall)
                    .map(|(b, v)| v > b + EPS),
            ),
        },
        Bar {
            id: "17.5",
            claim: "broker: identifica ≥ 80% dos testes de referência",
            baseline: None,
            value: broker.test_recall,
            verdict: judge(offline_ok, at_least(broker.test_recall, 0.80)),
        },
        Bar {
            id: "23.15.1",
            claim: "online × offline: mantém ou melhora a taxa de conclusão correta",
            baseline: broker.completion,
            value: online.completion,
            verdict: judge(online_ok, not_below(broker.completion, online.completion)),
        },
        Bar {
            id: "23.15.2",
            claim: "online × offline: +10 pp de recall dos arquivos apresentados onde o vocabulário diverge",
            baseline: div_off,
            value: div_on,
            verdict: judge(online_ok, at_least(gain, 0.10)),
        },
        Bar {
            id: "23.15.3",
            claim: "online × offline: ≥ 20% menos buscas e leituras",
            baseline: broker.exploratory,
            value: online.exploratory,
            verdict: judge(online_ok, at_least(explore_online, 0.20)),
        },
    ]
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "—".into(), |v| format!("{v:.3}"))
}

/// The report in Markdown, in the PRD's language.
pub fn render(records: &[RunRecord]) -> String {
    let mut out = String::from("# Avaliação A/B do ripwire-broker\n\n## Braços\n\n");
    out.push_str(
        "| braço | execuções | válidas | tarefas | repos | tokens | busca+leitura | bytes MCP | conclusão | recall arquivos | recall apresentados | recall testes | custo (US$) |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for arm in super::arm::ALL {
        let s = arm_stats(records, arm.name());
        if s.runs == 0 {
            continue;
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.2} |\n",
            s.arm,
            s.runs,
            s.valid,
            s.tasks,
            s.repos,
            num(s.tokens),
            num(s.exploratory),
            num(s.mcp_result_bytes),
            num(s.completion),
            num(s.file_recall),
            num(s.presented_recall),
            num(s.test_recall),
            s.cost_usd
        ));
    }
    out.push_str(&format!(
        "\n## Barras\n\nUma barra é `insuficiente` com menos de {MIN_TASKS} tarefas em {MIN_REPOS} repositórios \
         válidas nos dois braços que compara.\n\n| barra | afirmação | base | valor | veredito |\n| --- | --- | --- | --- | --- |\n"
    ));
    for b in bars(records) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | **{}** |\n",
            b.id,
            b.claim,
            num(b.baseline),
            num(b.value),
            b.verdict.label()
        ));
    }
    let invalid: Vec<&RunRecord> = records.iter().filter(|r| !r.valid).collect();
    if !invalid.is_empty() {
        out.push_str(&format!(
            "\n## Execuções inválidas ({}), fora das médias\n\n",
            invalid.len()
        ));
        for r in invalid {
            out.push_str(&format!(
                "- `{}` · {} · repetição {}: {}\n",
                r.task,
                r.arm,
                r.repeat,
                r.invalid.as_deref().unwrap_or("?")
            ));
        }
    }
    out.push_str(
        "\n## Cobertas pela suíte, não por esta medição\n\n\
         - §17.6 orçamento respeitado em 100% das respostas, §17.8 nenhuma rede no modo padrão e \
         §17.9 workspace intocado: `tests/props.rs`, `tests/mcp_surface.rs` e o CA-10 do CI.\n\
         - §23.15.4–6 (orçamento, inelegíveis, concorrência): `tests/online*.rs` e `tests/props*.rs`.\n\
         - §17.10 (< 50 ms p95 de processamento próprio): medido em D-072/D-073, não pelo agente.\n\
         - §23.15.7: o custo remoto do braço `broker-online` está em `status.online.metrics` do \
         servidor, não no transcript do agente; a coluna de custo acima é só a do agente.\n",
    );
    out
}
