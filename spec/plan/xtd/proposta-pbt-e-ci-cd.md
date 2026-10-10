# Proposta — testes de propriedade e CI/CD

Status: **cumprida — as sete fatias entregues em [D-107](../../changelog.md#d-107--ci-endurecido-dois-jobs-e-quatro-promessas-viram-portas) a [D-113](../../changelog.md#d-113--fatia-g-o-escalonador-e-a-quarta-vez-que-o-instrumento-era-o-problema); dois defeitos de segurança achados** · 2026-09-28 22:59, fechada 2026-09-29 00:08 · fonte:
[`spec/prompt/ci-cd.md`](../../prompt/ci-cd.md)

Este documento **não vira código antes de aprovação**. Ele faz três coisas: separa o que o prompt
afirma do que eu **verifiquei contra a árvore agora**; decide (com custo assumido) o entregável 1,
que bloqueia todo o resto; e fatia o trabalho em PRs com esforço e risco por fatia, dizendo onde o
esforço **não é estimável** e por quê.

## 1. O que foi verificado, não aceito

O prompt já se declara verificado contra o código, mas foi escrito **antes** das nove decisões
desta sessão ([D-097](../../changelog.md#d-097--teto-nos-três-caches-medido-antes-de-escolher-os-números) a
[D-106](../../changelog.md#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição)).
Refiz as medições que decidem esforço.

### Os quatro "controles de graça" passam limpos hoje

| controle | como foi conferido | resultado |
| --- | --- | --- |
| `#![forbid(unsafe_code)]` + `#![deny(clippy::print_stdout, clippy::dbg_macro)]` | atributos inseridos em `src/lib.rs`, `cargo clippy --all-targets --locked -- -D warnings` nas duas features | **passa nas duas**, zero avisos |
| porta do CA-10 | `cargo tree -e normal` | default: **zero** ocorrências de `reqwest`/`secrecy`/`rustls`/`hyper`; com `online`: 18 linhas |
| guarda de fixture sintética | `grep -rEil "Bearer \|api_key\|authorization\|sk-[A-Za-z0-9]{10}" tests/fixtures/` | **zero acertos** |
| `--locked` | `cargo metadata --locked` | `Cargo.lock` está em sincronia |

Adotá-los é escrever o passo, não consertar código. É a parte do prompt com melhor razão
valor/esforço.

### O `deny.toml` é uma allowlist curta, não uma auditoria

187 pacotes no grafo `online` (100 no default). Todas as licenças são permissivas —
`MIT OR Apache-2.0` (105), `MIT` (31), `Unicode-3.0` (18), `Apache-2.0 OR MIT` (11),
`Unlicense OR MIT` (5), `ISC`, `Zlib`, e uma `Apache-2.0 WITH LLVM-exception`.

> **Corrigido na fatia B ([D-108](../../changelog.md#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot)):**
> esta seção afirmava "nenhum crate duplicado em duas versões". **Errado** — o `cargo-deny`
> encontra quatro (`base64`, `getrandom`, `syn`, `windows-sys`). Meu pipeline de verificação
> deduplicava nome+versão e só então procurava linhas repetidas, então não podia achar nada.
> As quatro são pins transitivos, não acionáveis daqui, e ficam em `warn`.

### A superfície P0 pura é alcançável no build **default**

Provado por sonda de compilação (um `tests/zz_probe.rs` temporário que faz `use` de cada símbolo,
compilado **sem** a feature `online`, e depois removido): `workspace::Workspace`, `cli::parse`,
`notes::{sanitize,key}`, `local::CONTEXT_OPEN`, `model::Envelope` e **toda a superfície pura de
`online::*`** — `request`, `response`, `decision`, `redact`, `cache`, `reader::units`,
`retry_after`. A regra do prompt "o job de propriedades não liga a feature `online`" é viável de
fato, não por boa vontade.

### Três correções ao prompt

1. **A cobertura está desatualizada.** O prompt diz 234 (default) e 247 (`online`); hoje é
   **248 e 261**.
2. **P0.11 está desatualizado pelo [D-103](../../changelog.md#d-103--uma-nota-não-custa-mais-um-item-que-já-foi-evidência).**
   As entradas agora são ajustadas contra `requested_tokens - notes_reserve()` quando há
   summarizer, e o `next_step` é escrito **depois** das decisões de encaixe. Uma propriedade que
   afirme que o encaixe é **máximo** seria falsa — foi exatamente o que derrubou meu teste-ouro do
   D-099 ([D-102](../../changelog.md#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho)).
   O `estimated_tokens ≤ requested_tokens` continua valendo e é a forma certa de escrever a
   propriedade.
3. **O prompt não menciona que o job se chama `build`**, e é esse nome que a proteção de branch e
   o monitor de merge usam. Ver §5.

## 2. Entregável 1 — a decisão de visibilidade

Cinco módulos privados (`markup`, `normalize`, `router`, `budget`, `dedup`) e três caminhos, cada um
com custo. **Recomendação:**

> Promover **apenas `markup`** a `pub`, com uma linha de doc dizendo que é interno e **não carrega
> promessa de estabilidade**. Manter `normalize`, `router`, `budget` e `dedup` privados e
> exercitá-los pela costura pública `Broker` + `FakeUpstream`.

Razão: dos cinco, `markup` é o **único que lê bytes de fora do processo** — a saída do ripwire. É
por isso que P0.12 é controle de disponibilidade e não higiene, e é por isso que ele já teve dois
panics ([D-091](../../changelog.md#d-091--revisão-do-repositório-e-correções-de-robustez)). Fuzzar um parser
*através* de um `FakeUpstream` significa que cada caso gerado atravessa spawn, JSON-RPC e
normalização — ordens de magnitude mais lento, e a propriedade deixa de ser "para qualquer `&str`"
para ser "para qualquer `&str` que sobreviva ao transporte". Isso não fecha a classe.

Os outros quatro não têm esse problema: são alimentados pelo `markup` e o próprio prompt já
prescreve P0.11 pela costura pública. Uma linha de superfície nova, pelo único caso que a justifica.

**A opção 3 (`#[cfg(test)] mod tests` em `src/`) está descartada** — o repositório não tem nenhum
por convenção deliberada, e um teto no `Inflight` já foi revertido para preservá-la
([D-093](../../changelog.md#d-093--fechamento-das-ressalvas-do-install-e-do-inflight) → [D-094](../../changelog.md#d-094--reversão-do-teto-do-inflight)).
Não quebro isso por conveniência de teste.

## 3. Prioridade, com os controles de segurança no topo

Ordem pelos itens do prompt, não uma tabela nova.

| ordem | item | por que aí |
| --- | --- | --- |
| 1 | **P0.7** `redact::remote_text` | confidencialidade: última barreira antes de texto remoto chegar a status, log ou agente. Puro, rápido, propriedade absoluta ("nunca contém o segredo") |
| 2 | **P0.5** `parse_answers` | disponibilidade + correção: entrada de fora do processo; "nunca vira `false`, nunca é truncada para dentro da faixa" |
| 3 | **P0.12** `markup::parse` | disponibilidade: panic = negação de serviço por stdout alheio, e já aconteceu duas vezes. Depende de §2 |
| 4 | **P0.1** guarda de workspace | path traversal (RF-02, CA-08). Precisa de tempdir |
| 5 | **P0.2** elegibilidade | não enviar arquivo sensível. Precisa de tempdir e do crate `ignore` |
| 6 | **P0.13** escapamento do bloco `prompt` | injeção de prompt (D-052). Puro e novo |
| 7 | P0.8, P0.9 | chaves de cache e saneamento de nota; puros e baratos |
| 8 | P0.4, P0.6, P0.10 | lotes, limiares estritos, `cli::parse`; puros |
| 9 | P0.3 | unidades de evidência; tempdir, a propriedade mais intricada (contiguidade + cobertura + fronteira de caractere) |
| 10 | P1.1, P1.2, P1.3 | `retry_after`, ordenação, escalonador |

P1.4 **não entra**: `the_build_has_no_network_stack` já existe em `tests/mcp_surface.rs`, e o passo
de `cargo tree` do §1 o complementa como porta de CI. Duplicar seria ruído.

## 4. Fatias, esforço e risco

| # | PR | esforço | risco |
| --- | --- | --- | --- |
| A | CI: dois jobs, `permissions`, actions por SHA, `persist-credentials: false`, `--locked`, `timeout-minutes`, `concurrency`, `nextest` com filtro; os dois lints; passo do CA-10; guarda de fixture; `.env`/`.envrc` no `.gitignore` | **mecânico**, uma sessão | operacional, ver §5 |
| B | `deny.toml` + `.github/dependabot.yml` | pequeno | **o único desconhecido real**: `advisories` consulta uma DB viva e pode nascer vermelha por um crate transitivo que eu não controlo. É exatamente por isso que o prompt manda **agendar no `main`, não bloquear PR** |
| C | scaffolding do `proptest` + P0.7, P0.5, P0.8, P0.9 | ~12 a 18 propriedades (estimativa) | baixo |
| D | P0.4, P0.6, P0.10, P0.13, P1.1, P1.2 | ~12 a 18 propriedades (estimativa) | baixo |
| E | §2 + P0.12 + P0.11 pela costura pública | pequeno **em código** | **alto em achados** |
| F | P0.1, P0.2, P0.3 (tempdir) | poucas propriedades, muito cuidado | **alto**: `PROPTEST_CASES` baixo, `write_executable()` por causa do ETXTBSY ([D-088](../../changelog.md#d-088--ci-revisão-tdd-e-correções)), e uma falha aqui é **vulnerabilidade**, não bug de formatação |
| G | P1.3 escalonador com o `FakeClassifier` que já existe | pequeno | baixo |

### O que não é estimável, dito claramente

**O custo dominante não é escrever as propriedades — são os achados.** O andaime (A a D) é
previsível. As fatias E e F são deliberadamente apontadas a duas fronteiras que já falharam antes, e
cada achado é correção + teste + entrada `D-NNN` + PR própria. Dar um número total aqui seria
inventar; o que dá para dizer é que a parte previsível são ~4 PRs pequenas e que a imprevisível é
proporcional a quão defeituosas as fronteiras realmente são — que é o motivo de fazer o trabalho.

## 5. O perigo operacional que o prompt não vê

O workflow tem hoje **um** job chamado `build`. É esse nome literal que a proteção de branch usa
como status check e que o monitor de merge desta sessão exige com bucket `pass` — a checagem que
tive de endurecer depois de ela ter mesclado os PRs #10 e #11 com o job `build` ainda inexistente,
deixando o `master` vermelho
([D-102](../../changelog.md#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho)).

Reescrever em dois jobs **renomeia os checks**. Se isso for mesclado sem atualizar os status checks
obrigatórios, o merge seguinte passa **sem porta nenhuma**. Então a PR A tem de vir com:

1. os nomes finais dos dois jobs decididos antes (proposta: `default` e `online`);
2. a atualização da proteção de branch no `master` como parte da entrega, não depois;
3. um nome estável, porque trocar de novo repete o problema.

## 6. Regras de segurança do próprio suíte, que eu assumo

Do prompt, e concordo com todas: raiz **sempre** em `tempfile::TempDir`, nunca `$HOME` nem a raiz do
repo; **nenhuma** escrita ou remoção com caminho gerado fora do tempdir, em particular nada de
`remove_dir_all` com entrada de `proptest`; **nunca executar conteúdo gerado** — `write_executable()`
é para fixtures do suíte, não para bytes gerados; tamanhos gerados com teto (dezenas de KiB, com os
limiares de `MAX_READ_BYTES`/`LOCATION_ONLY_BYTES` cobertos por poucos casos dirigidos);
`.proptest-regressions/` versionada **mas revisada arquivo por arquivo** antes de commitar, com a
mesma regra do corpus do Jev; **nenhuma propriedade toca a rede**, e o job de propriedades não liga a
feature `online` — o §1 provou que não precisa.

## 7. O que rejeito por padrão

Como o prompt manda: conformidade de protocolo do `rust-mcp-sdk`, sessão MCP ao vivo como PBT,
comportamento de `reqwest`/TLS/DNS, internos de `serde`/`sha2`/`ignore`, e reproduzir o corpus
congelado de prompts como propriedade — `prompts_v1_are_frozen_for_*` e companhia são golden **de
propósito**, mudar o texto tem de quebrá-los.

SBOM e `harden-runner` ficam como **opcionais**, e digo que são. O `harden-runner` tem apelo real
aqui justamente porque o build default promete não ter pilha de rede, mas não é entrega obrigatória.

## 8. O que precisa de aprovação antes de qualquer código

1. A decisão de visibilidade do §2 — `markup` a `pub` com doc de "interno", os outros quatro pela
   costura pública.
2. Os nomes dos dois jobs e o compromisso de atualizar a proteção de branch **na mesma PR** (§5).
3. A política de pin no dependabot: `rust-mcp-sdk = "=2.0.0"` é exato de propósito e **só sobe por
   decisão registrada no changelog**, nunca por bump automático.
4. Se a revisão passa a ser obrigatória no `master`. O histórico tem PR mesclado sem revisão; se a
   intenção é manter assim, isso vira decisão registrada em vez de ficar implícito.

Sugiro começar pela **fatia A**: é independente, verificadamente barata (§1) e transforma quatro
promessas do PRD em portas de CI hoje.
