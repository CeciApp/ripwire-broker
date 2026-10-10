# Plano — S3.15: cache persistente de notas

Status: **implementado** (D-168) · 2026-10-10

Fonte: PRD §10.3 e §21.3; `plan-fases-2-3.md` (S3.15, adiada no D-046);
`plano-ab-e-session-hits.md` (a regra de decisão).

## 1. Por que agora

O §21.3 pedia medir antes de criar o cache. Medido nesta máquina em 2026-10-10 com `hook-stats`:
17 sessões, 85,2% de acerto dentro da sessão e **44,6% de repetição entre sessões** (160 de 359
fingerprints). A regra proposta põe o S3.15 no plano a partir de 30%, depois de 20 sessões; o
mantenedor decidiu implementar com 17.

O que o cache em memória perde: toda nota morre com o processo. Um `serve` novo paga de novo, ao
modelo local, a nota de um módulo que não mudou.

## 2. Decisões

| # | Decisão | Motivo |
|---|---|---|
| 1 | **Opt-in**: `--summarizer-cache`, só com `--summarizer-cmd` | grava em disco texto escrito por um modelo sobre o código; quem não pede não ganha um arquivo novo (PRD §16.1) |
| 2 | Arquivo `<state-dir>/notes/<sha256 da raiz>.json`, um por workspace | fora do repositório, como todo estado; um workspace nunca lê as notas de outro |
| 3 | A chave não muda: `hash(prompt, modelo, escopo, evidência)` | endereçada por conteúdo: evidência, prompt ou modelo diferente dão outra chave, então uma nota velha nunca é servida |
| 4 | O arquivo é **dado não confiável**: aberto sem seguir link e sem bloquear, só arquivo regular, teto de tamanho, chave em hex de 64, texto passado de novo pelo `sanitize`, no máximo `MAX_CACHED_NOTES` | quem escreve no state dir não ganha um caminho para injetar texto além do que o modelo já podia |
| 5 | Arquivo ilegível, corrompido ou de outra versão vale como vazio, e é reescrito na próxima nota | um cache nunca derruba nem atrasa a resposta |
| 6 | Escrita inteira a cada nota gerada, por `write_private` (temporário 0600 + rename, diretório 0700), fora da thread assíncrona | leitores veem o arquivo antigo ou o novo; no máximo ~350 KiB |
| 7 | Falha de escrita é silenciosa para a resposta: o cache segue em memória | idem 5 |
| 8 | Dois servidores no mesmo workspace: o último a escrever vence | sem lock nem merge; perde-se no máximo notas que o outro regeneraria |
| 9 | O status diz `persistent: true` e `cached_notes` conta as carregadas | o usuário vê que está ligado, sem conteúdo |

Fora de escopo: os hooks (o S3.17 saiu no D-046; um hook não roda modelo), expiração por idade (a
chave por conteúdo já invalida; o teto de 500 já limita) e um comando para apagar (é um arquivo:
`rm`).

## 3. Ciclos (vermelho → verde)

| # | Teste | Costura | Falha antes porque |
|---|---|---|---|
| 1 | `the_note_cache_survives_a_restart_and_ignores_corrupt_files` | `tests/notes.rs` (dois `Broker`, mesmo diretório) | o segundo `Broker` chama o modelo de novo |
| 2 | `without_a_cache_dir_nothing_is_written` | idem | guarda: já passa, e tem que continuar passando |
| 3 | `the_cache_file_is_private_and_belongs_to_one_workspace` | idem | não há arquivo |
| 4 | `a_tampered_cache_file_is_sanitized_capped_and_never_read_through_a_link` | idem | não há leitura |
| 5 | `the_status_says_when_the_note_cache_is_persistent` | idem | não há o campo |
| 6 | `serve_takes_summarizer_cache_only_with_a_summarizer` | `tests/cli.rs` | `unknown argument` |

Controle negativo de cada um: tirar a linha que o faz passar e ver o vermelho voltar.

## 4. Documentação

README (tabela de flags e a seção das notas), `USAGE`, PRD §21.3, `plan-fases-2-3.md` (a linha do
S3.15), `handoff.md`, changelog D-168.
