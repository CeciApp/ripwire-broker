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
    /// `None` when the task's reference names no tests.
    #[serde(default)]
    pub test_recall: Option<f64>,
    pub correct: Option<bool>,
    /// An earlier session of its sequence was invalid: the memory this one started from is not
    /// the history the corpus describes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub history_incomplete: bool,
    /// The agent's version and model, from its session (PRD jev-mem §14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// `context_for_task` latency as the agent waited for it, summed over the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_for_task_ms: Option<u64>,
    /// Memory arms only (PRD jev-mem §14), never a zero for the others: what the session's reads
    /// sent and delivered...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_retrieval_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_retrieval_questions: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memories_delivered: Option<u64>,
    /// ...and what the store's 24-hour quota grew by during the session besides them: the
    /// worker's enrichment and consolidation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_ingestion_attempts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_ingestion_questions: Option<u64>,
    /// Observations and jobs the session left for the next process: its ingestion is paid by
    /// the next session of the sequence, or by none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_jobs_left: Option<u64>,
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

/// A run that counts in the averages: valid, and for a memory arm, started from the history the
/// corpus describes.
fn counts(r: &RunRecord) -> bool {
    r.valid && !r.history_incomplete
}

fn valid<'a>(records: &'a [RunRecord], arm: &str) -> Vec<&'a RunRecord> {
    records
        .iter()
        .filter(|r| r.arm == arm && counts(r))
        .collect()
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
        // A broker arm that presented nothing presented a recall of 0, as the bars count it;
        // an arm without the broker has no such measure.
        presented_recall: match broker_arm(arm) {
            true => f(presented),
            false => mean(v.iter().filter_map(|r| r.presented_recall)),
        },
        test_recall: mean(v.iter().filter_map(|r| r.test_recall)),
    }
}

fn broker_arm(arm: &str) -> bool {
    super::arm::Arm::parse(arm).is_some_and(|a| a.server() == Some("ripwire-broker"))
}

/// The runs of arms `a` and `b` on the (task, repeat) pairs valid in both: a comparison is over
/// the same tasks, or a task one arm failed to run would weigh on one side only.
fn paired(records: &[RunRecord], a: &str, b: &str) -> Vec<RunRecord> {
    let keys = |arm| {
        valid(records, arm)
            .into_iter()
            .map(|r| (r.task.clone(), r.repo.clone(), r.repeat))
            .collect::<BTreeSet<_>>()
    };
    let both: BTreeSet<_> = keys(a).intersection(&keys(b)).cloned().collect();
    records
        .iter()
        .filter(|r| counts(r) && (r.arm == a || r.arm == b))
        .filter(|r| both.contains(&(r.task.clone(), r.repo.clone(), r.repeat)))
        .cloned()
        .collect()
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
    // Each comparison over the runs both of its arms have.
    let offline = paired(records, "none", "broker");
    let online_pair = paired(records, "broker", "broker-online");
    let none = arm_stats(&offline, "none");
    let broker = arm_stats(&offline, "broker");
    let broker_on = arm_stats(&online_pair, "broker");
    let online = arm_stats(&online_pair, "broker-online");
    let offline_ok = sufficient(records, "none", "broker");
    let online_ok = sufficient(records, "broker", "broker-online");

    let tokens = reduction(none.tokens, broker.tokens);
    let explore = reduction(none.exploratory, broker.exploratory);
    let explore_online = reduction(broker_on.exploratory, online.exploratory);

    let divergent = |arm: &str| {
        mean(
            valid(&online_pair, arm)
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
            baseline: broker_on.completion,
            value: online.completion,
            verdict: judge(
                online_ok,
                not_below(broker_on.completion, online.completion),
            ),
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
            baseline: broker_on.exploratory,
            value: online.exploratory,
            verdict: judge(online_ok, at_least(explore_online, 0.20)),
        },
    ]
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "—".into(), |v| format!("{v:.3}"))
}

/// Memory's cost for one arm, averaged over its valid runs (PRD jev-mem §14): retrieval,
/// ingestion and the agent's own latency apart. `None` where the arm has no such measure.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MemoryCost {
    pub arm: String,
    pub valid: usize,
    pub retrieval_requests: Option<f64>,
    pub retrieval_questions: Option<f64>,
    pub delivered: Option<f64>,
    pub context_for_task_ms: Option<f64>,
    pub ingestion_attempts: Option<f64>,
    pub ingestion_questions: Option<f64>,
    pub jobs_left: Option<f64>,
    pub agent_ms: Option<f64>,
}

/// The memory arms, and arm A beside them for its wait on `context_for_task`. Empty without a
/// memory arm.
pub fn memory_cost(records: &[RunRecord]) -> Vec<MemoryCost> {
    let arms: Vec<_> = super::arm::ALL
        .into_iter()
        .filter(|a| a.memory() || *a == super::arm::Arm::BrokerOnline)
        .filter(|a| !valid(records, a.name()).is_empty())
        .collect();
    if !arms.iter().any(|a| a.memory()) {
        return vec![];
    }
    arms.into_iter()
        .map(|arm| {
            let v = valid(records, arm.name());
            let f = |g: fn(&RunRecord) -> Option<u64>| {
                mean(v.iter().filter_map(|r| g(r)).map(|n| n as f64))
            };
            MemoryCost {
                arm: arm.name().into(),
                valid: v.len(),
                retrieval_requests: f(|r| r.memory_retrieval_requests),
                retrieval_questions: f(|r| r.memory_retrieval_questions),
                delivered: f(|r| r.memories_delivered),
                context_for_task_ms: f(|r| r.context_for_task_ms),
                ingestion_attempts: f(|r| r.memory_ingestion_attempts),
                ingestion_questions: f(|r| r.memory_ingestion_questions),
                jobs_left: f(|r| r.memory_jobs_left),
                agent_ms: f(|r| Some(r.duration_ms)),
            }
        })
        .collect()
}

fn render_memory_cost(records: &[RunRecord]) -> String {
    let rows = memory_cost(records);
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Custo da memória\n\nMédias por execução válida. Recuperação: o que as leituras de \
         memória enviaram ao classificador e entregaram, e a espera do agente por \
         `context_for_task` (estrutura e memória juntas; o braço A, sem memória, fica ao lado para \
         comparar). Ingestão: o quanto a quota de 24 h do store da rodada cresceu na sessão além das \
         leituras (enriquecimento e consolidação). O worker morre com a sessão: o que ela deixa (jobs \
         pendentes ao fim) é pago pela sessão seguinte da sequência, ou por nenhuma, então a ingestão \
         de cada sessão inclui a da anterior. Perguntas não viram dólares sem preço verificado (PRD \
         jev-mem §8.3).\n\n\
         | braço | válidas | leitura: requests | leitura: perguntas | memórias entregues | \
         context_for_task (ms) | ingestão: tentativas | ingestão: perguntas | jobs pendentes ao fim \
         | agente (ms) |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for c in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            c.arm,
            c.valid,
            num(c.retrieval_requests),
            num(c.retrieval_questions),
            num(c.delivered),
            num(c.context_for_task_ms),
            num(c.ingestion_attempts),
            num(c.ingestion_questions),
            num(c.jobs_left),
            num(c.agent_ms),
        ));
    }
    out
}

/// What the runs were made with (PRD jev-mem §14): the run's `versions.json`, and the agent
/// versions its sessions announced.
pub fn render_versions(v: &serde_json::Value, records: &[RunRecord]) -> String {
    let mut out = String::from("\n## Versões\n\n");
    for (key, label) in [
        ("ripwire_broker", "ripwire-broker"),
        ("ripwire", "ripwire"),
        ("jev_model", "modelo Jev"),
        ("summarizer", "sumarizador"),
    ] {
        let value = v[key].as_str().unwrap_or("—");
        out.push_str(&format!("- {label}: `{value}`\n"));
    }
    let agents: BTreeSet<&str> = records.iter().filter_map(|r| r.agent.as_deref()).collect();
    let agents: Vec<String> = agents.iter().map(|a| format!("`{a}`")).collect();
    let agents = match agents.is_empty() {
        true => "não anunciado".to_string(),
        false => agents.join(", "),
    };
    out.push_str(&format!("- agente: {agents}\n"));
    out
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
    out.push_str(&render_memory_cost(records));
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
    let partial: Vec<&RunRecord> = records
        .iter()
        .filter(|r| r.valid && r.history_incomplete)
        .collect();
    if !partial.is_empty() {
        out.push_str(&format!(
            "\n## Sessões com histórico incompleto ({}), fora das médias\n\n\
             Uma sessão anterior da sequência foi inválida: a memória de partida não é a do corpus.\n\n",
            partial.len()
        ));
        for r in partial {
            out.push_str(&format!(
                "- `{}` · {} · repetição {}\n",
                r.task, r.arm, r.repeat
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
