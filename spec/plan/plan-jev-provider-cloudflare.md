# Plano: `--jev-provider cloudflare` (TDD)

Spec: `spec/prompt/jev-provider-cloudflare.md`. Cada ciclo abaixo é **red → green → refactor**:
o teste entra primeiro, roda, falha pelo motivo esperado, e só então vem o código. Nada de
`request.rs` nem de `parse_answers` muda (restrição do spec); se um ciclo parecer exigir isso,
pare e explique.

Comando de verificação de cada ciclo: `cargo test --features online --test <arquivo> <nome>`.
Ao fim: `cargo test --features online`, `cargo test` (sem feature: `cli.rs` continua compilando
porque `JevProvider` fica fora do gate `online`), `ripwire --quality-delta`, `ripwire --test-gate`.

## 0. O que existe hoje (âncoras)

| Peça | Onde | O que faz hoje |
|---|---|---|
| Endpoint fixo | `src/online/jev.rs:14` `ENDPOINT` | Único URL; `new`/`without_key` passam ele a `build` |
| Construtores injetáveis | `src/online/jev.rs:42-78` `loopback*` | Já provam que o endpoint é parâmetro de `build` |
| POST e leitura | `src/online/jev.rs:184-290` `post`/`exchange` | Devolve `String` do corpo; `classify` e `decide` chamam `parse_answers`/`parse_decisions` sobre ela |
| Erro de resposta | `src/online/response.rs:8` `InvalidResponse` | Enum `Copy`, sem payload |
| Flags online | `src/cli.rs:119-143` `OnlineArgs` (`provider: String`) e `src/cli.rs:367-416` `Flags::online` | Recusa tudo que não seja `typesafe` |
| Lista de `--jev-*` valorados | `src/cli.rs:499-509` `JEV` | Onde `--jev-account-id` entra |
| Doctor / drain / install | `src/cli.rs:813-838`, `:903-945`, `:840-868` | Só `--jev-model`; nenhum carrega provider |
| Construção do cliente | `src/main.rs:139,150` (serve), `:214` (drain), `src/doctor.rs:229` (probe) | `JevClient::new(key, model, timeout)` |
| Host no status e no cache | `src/online/coordinator.rs:27,50` `endpoint_host` | Hardcoded `api.typesafe.ai`; `main.rs` não sobrescreve |
| Log | `src/online/log.rs:97` `POST {endpoint}` | Imprime a URL inteira |
| Redação | `src/online/redact.rs` `remote_text` | Só a credencial |
| Snippet do install | `src/install.rs:405-420` (`.mcp.json`), `:525-545` (Codex) | Só `--online`/`--memory*` |
| Fixture loopback | `tests/online_protocol.rs:36-115` | Servidor HTTP/1.1 em 127.0.0.1, respostas enfileiradas |

## 0b. O que a doc da Cloudflare diz (verificado em 2026-10-09)

Fontes: página do modelo `@cf/cloudflare/clef` e `clef-flash`, página de preços do Workers AI,
changelog de 01/10/2026, guia REST do Workers AI, artigo flaviocopes.com/clef, e a issue
pydantic/pydantic-ai#9765 (relato de uso real do REST).

| Ponto | Spec diz | Doc diz | Efeito no plano |
|---|---|---|---|
| Endpoint | `.../accounts/<ID>/ai/run/@cf/cloudflare/<MODEL>` | Idêntico. Não há rota `/v1/systemone` na Cloudflare | Confirma ciclo 3 |
| `model` no corpo | `clef` ou `clef-flash` | Obrigatório, padrão `^\s*(clef|clef-flash)\s*$`, e **tem que bater com o da URL**; divergência devolve `422 Unsupported model 'clef'. Use 'clef-flash'` | Validar `--jev-model` na CLI (decisão 4b) |
| Envelope | `{success, errors, messages, result:{model, answers, usage}}` | Idêntico no REST; o binding de Worker devolve a raiz nua | Confirma decisão 2 e o teste "raiz nua continua aceita" |
| Erro de pedido | "erro 5006" | 5006 é o código de `model` fora do padrão. Erros vêm com **status 4xx** (422 observado), corpo `{"success":false,"errors":[...]}`; campos extras no corpo dão 422 `Extra inputs are not permitted` | Ciclo 2 ganha o caso 4xx (abaixo) |
| Contexto | 65.536 tokens | `clef`: 65.536. `clef-flash`: a página do modelo diz **24.576**, o changelog diz 64K | README cita os dois números e a fonte; limite seguro é 24.576 |
| Preço clef-flash | 0,09 $/Mtok | **0,038 $/Mtok** (página de preços e página do modelo, 3.455 neurons/Mtok). 0,09 era o valor do artigo de 01/10 | README e cálculo do probe usam 0,038 |
| Preço clef | 0,24 $/Mtok | 0,24 (21.818 neurons/Mtok). Saída não cobrada nos dois | Confirma |
| Token | Workers AI Read+Edit | Confirma | — |
| `usage` | dentro de `result` | Dentro de `result`, com `input_tokens`/`output_tokens`; `output_tokens` observado sempre 0 | Confirma |
| Content-Type da resposta | — | **Não documentado.** `exchange` exige `application/json` (`jev.rs:244`) | Risco; o live test do ciclo 8 é quem prova |
| `questions` por pedido | — | 1 a 64 | `--jev-request-limit` não é afetado; conferir que um lote nunca passa de 64 (ver `scheduler.rs`) |

## 1. Decisões de desenho (antes de codar)

1. **`JevProvider` é um enum**, em `src/online/mod.rs` (fora do `#[cfg(feature = "online")]`,
   como `DEFAULT_MODEL`, porque `cli.rs` compila sem a feature):
   `TypeSafe | Cloudflare`, com `name() -> &'static str` (`typesafe`/`cloudflare`),
   `parse(&str)`, `host()` e `endpoint(&self, account: Option<&str>, model: &str) -> String`.
   `endpoint` é função pura: é ela que o teste de URL mede.
   `OnlineArgs.provider` vira `JevProvider` e ganha `account_id: Option<String>`. O
   `OnlineConfig.provider: String` continua string (`name()`), para o cache
   (`cache.rs:15`), a proveniência (`merge.rs:374`) e o status (`mcp.rs:238`) não mudarem de tipo.
2. **Desembrulho vive em `JevClient::post`**, entre `exchange` e o retorno, numa função
   `unwrap_envelope(provider, text) -> Result<String, InvalidResponse>`:
   - `TypeSafe`: identidade. Regressão byte a byte garantida por construção.
   - `Cloudflare`: se o JSON tem `success` e `result`, devolve `result` re-serializado;
     `success: false` → `InvalidResponse::Refused(errors)`; raiz sem envelope passa inteira.
   Assim `classify` e `decide` (memória) herdam o desembrulho, e `parse_answers` não aprende nada.
3. **`InvalidResponse::Refused(String)`** carrega os `errors` serializados, passados por
   `redact::remote_text(.., key, 256)` antes. Isso tira o `Copy` do enum: rodar
   `ripwire . --impact=InvalidResponse --legend=compact` antes do ciclo 2 e ajustar os sítios que
   dependem de cópia (hoje 29 usos fora de `response.rs`). É mudança no *tipo* de `response.rs`,
   não em `parse_answers`; registrar isso no PR. Alternativa, se o impacto for grande demais:
   `ClassifyError::Refused { errors }` em `classifier.rs` (que já não é `Copy`). Decidir no ciclo 2
   com o número na mão.
4. **Modelo padrão por provider.** `DEFAULT_MODEL` (`jev-1.13.0`) na Cloudflare devolve 5006
   sempre. Proposta: `JevProvider::default_model()` → `jev-1.13.0` / `clef-flash`; `--jev-model`
   continua mandando. Assunção a confirmar com o autor do spec; custo de trocar depois é uma linha.
4b. **Modelo validado na CLI para a Cloudflare.** A doc fixa o padrão `clef|clef-flash` e exige
   que corpo e URL batam; como o broker monta os dois do mesmo `--jev-model`, a única falha
   possível é um nome fora do padrão, e ela é certa (5006/422). `Flags::provider()` recusa
   `--jev-provider cloudflare --jev-model jev-1.13.0` com usage ("a Cloudflare aceita clef ou
   clef-flash"). Para TypeSafe nada é validado, como hoje.
5. **Account id no log: redigir.** Não é credencial, mas identifica o tenant e o log existe para
   ser lido e colado por uma pessoa. Custo de redigir é zero; custo de vazar é um identificador
   estável a mais. Fica `accounts/[redacted]/`, via `redact::endpoint_for_log(url)` em
   `redact.rs`, aplicada em `jev.rs` ao montar `Exchange.endpoint`. O moduledoc de `log.rs`
   ganha a frase. `endpoint_host` no status mostra só o host (`api.cloudflare.com`), sem id.
6. **Fora do produto:** `tests/online_live.rs` lê provider e account id de variáveis de teste
   (`RIPWIRE_BROKER_LIVE_PROVIDER`, `RIPWIRE_BROKER_LIVE_ACCOUNT_ID`); o binário não ganha
   variável nova (spec item 5).

## 2. Ciclos TDD, na ordem obrigatória do spec

### Ciclo 1 — o teste que falha hoje (envelope)

Arquivo: `tests/online_protocol.rs`.

1. Scaffold mínimo para compilar (sem comportamento): `JevProvider` enum em `mod.rs` com só
   `TypeSafe | Cloudflare`, e `JevClient::loopback_with(port, provider, key, model, timeout)` que
   guarda o provider num campo novo `provider: JevProvider` de `JevClient` e **não o usa**.
   `loopback`, `loopback_h2`, `loopback_without_key` passam a delegar com `TypeSafe`; sem mudança
   observável.
2. Teste `a_cloudflare_envelope_is_unwrapped_before_the_answers`:
   ```rust
   const CF_OK: &str = r#"{"success":true,"errors":[],"messages":[],"result":{"model":"clef-flash","answers":{"q0":{"type":"noul","noul":0.81},"q1":{"type":"noul","noul":0.03}},"usage":{"input_tokens":10,"output_tokens":2}}}"#;
   ```
   Fixture responde `json(200, CF_OK)`; cliente `loopback_with(port, Cloudflare, key, "clef-flash", ..)`;
   request construído com modelo `clef-flash`; asserção `answers == [Some(0.81), Some(0.03)]`.
3. Rodar. **Deve falhar com `Invalid(Malformed)`**: `parse_answers` não acha `model`/`answers` na
   raiz. Anotar a saída (vai para o PR).
4. Green: `unwrap_envelope` em `jev.rs`, chamado em `post`. Rodar: passa.
5. Mesmo teste com `MemoryClassifier::decide` (um `StateRequest` mínimo) para cobrir o caminho da
   memória, que compartilha `post`.

### Ciclo 2 — controle negativo (`success: false`)

1. Teste `a_cloudflare_refusal_is_invalid_with_its_errors`: corpo
   `{"success":false,"errors":[{"code":5006,"message":"model not found"}],"messages":[],"result":null}`,
   HTTP 200. Asserção: `Err(ClassifyError::Invalid(InvalidResponse::Refused(m)))` com
   `m.contains("5006")` e `m.contains("model not found")`.
2. Rodar antes de implementar `Refused`: não compila (variante não existe). Adicionar a variante
   e rodar de novo: **deve falhar** porque sem o ramo `success: false` o desembrulho devolve
   `result: null` e cai em `Malformed`. Se passar aqui, a asserção não mediu nada; parar.
3. Green: ramo `success == false` em `unwrap_envelope`; `errors` serializados compactos, passados
   por `remote_text` com a chave para redigir, cap 256.
4. Teste de redação: o `errors` ecoa `tok-123`; a mensagem não pode conter `tok-123`.
5. Teste `a_cloudflare_bare_root_is_still_accepted`: resposta no formato TypeSafe, provider
   Cloudflare, continua aceita (spec item 4).
6. `ClassifyError::category()` para `Refused` → `invalid_response` (categoria existente,
   `classifier.rs:80`; status e log só veem a categoria).
7. **O caso real: 4xx com envelope.** A doc mostra que a Cloudflare recusa com status 422 e
   `{"success":false,"errors":[...]}`, não com 200. Hoje `exchange` (`jev.rs:217-235`) mapeia o
   status antes de ler o corpo e devolve `Rejected(422)`, sem texto; o corpo só é lido com `--log`.
   Teste `a_cloudflare_4xx_keeps_its_status_and_logs_its_errors`: fixture responde
   `json(422, "{\"success\":false,\"errors\":[{\"code\":5006,\"message\":\"...\"}]}")`; asserções:
   `Err(ClassifyError::Rejected(422))` **e**, com `--log`, o bloco `recebido` contém `5006`.
   Esse teste passa já hoje: é um teste de caracterização, para fixar que o 4xx não entra no
   desembrulho e que o erro fica legível no log. Não mudar o mapeamento de status: a
   categoria `rejected` já alimenta retry e status, e o spec não pede texto no erro.

### Ciclo 3 — URL e CLI

Arquivo: `tests/cli.rs` (parse puro, sem rede) e um teste unitário em `src/online/mod.rs`.

1. Teste da URL: `JevProvider::Cloudflare.endpoint(Some("abc123"), "clef-flash")` ==
   `https://api.cloudflare.com/client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef-flash`;
   `TypeSafe.endpoint(None, "jev-1.13.0")` == `https://api.typesafe.ai/v1/systemone`
   (= `ENDPOINT`, que fica como constante). Red: método não existe. Green: implementar.
2. Testes de CLI (`bad_online_flags_are_usage_errors` ganha casos, mais um teste positivo):
   - `--online --jev-provider cloudflare` sem `--jev-account-id` → usage, mensagem cita
     `--jev-account-id`.
   - `--online --jev-provider typesafe --jev-account-id x` → usage, mensagem diz que só a
     Cloudflare aceita.
   - `--online --jev-account-id x` (provider implícito typesafe) → usage.
   - `--jev-provider other` → mensagem agora lista `typesafe|cloudflare`.
   - Positivo: `--online --jev-provider cloudflare --jev-account-id abc123 --jev-model clef-flash`
     → `OnlineArgs { provider: Cloudflare, account_id: Some("abc123"), model: "clef-flash", .. }`.
   - Padrão: `--online` sozinho → `provider: TypeSafe, account_id: None, model: "jev-1.13.0"`
     (atualizar o literal em `tests/cli.rs:289`).
   - Sem `--jev-model` com cloudflare → `model == "clef-flash"` (decisão 4).
   - `--jev-provider cloudflare --jev-account-id x --jev-model jev-1.13.0` → usage que cita
     `clef` e `clef-flash` (decisão 4b). `--jev-model " clef "` com espaços: a doc tolera, mas
     a URL não; recusar também (o padrão exige igualdade exata).
3. Green em `cli.rs`: `--jev-account-id` entra em `JEV`; `Flags::provider()` novo, devolve
   `(JevProvider, Option<String>)` com as duas validações, e `Flags::online` o usa. `USAGE`
   atualizado (`[--jev-provider typesafe|cloudflare] [--jev-account-id ID]`) e a linha da
   credencial vira "chave TypeSafe ou token Cloudflare, conforme `--jev-provider`".
4. `doctor`, `memory drain`, `install` (mesmo ciclo, um teste de parse cada):
   - `doctor --jev-probe --jev-provider cloudflare --jev-account-id x` → `DoctorArgs` ganha
     `jev_provider`, `jev_account_id`; sem `--jev-probe` os dois são usage, como `--jev-model`.
   - `memory drain --online --jev-provider cloudflare --jev-account-id x` → `MemoryAction::Drain`
     ganha os dois campos.
   - `install claude-code --online --jev-provider cloudflare --jev-account-id x` → `InstallArgs`
     ganha os dois; teste existente do `.mcp.json` (`tests/cli.rs:1942`) ganha um irmão que
     espera `args` com `--jev-provider`, `cloudflare`, `--jev-account-id`, `x`; idem para o TOML
     do Codex (`tests/cli.rs:1978`). Sem `--online`/`--memory`, os dois são usage.
   - Todos os quatro parsers chamam o mesmo `Flags::provider()`: a validação existe uma vez.

### Ciclo 4 — ligar o provider ao cliente

1. `JevClient::new(provider, account_id, key, model, timeout)` e `without_key(provider, account_id, model, timeout)`:
   endpoint vem de `provider.endpoint(account_id, model)`. Chamadores: `main.rs:139,150,214`,
   `doctor.rs:229`, `online_live.rs:31`. `build` ganha o `provider` que o ciclo 1 já guardou.
2. `main.rs` `online_config`: `config.provider = o.provider.name().into()` e
   `config.endpoint_host = o.provider.host().into()` (hoje nunca é sobrescrito: sem isso o
   status e a chave do cache diriam `api.typesafe.ai` para a Cloudflare). Teste em
   `tests/online.rs` ou `tests/server_status.rs`: status `online.provider == "cloudflare"` e
   `endpoint_host == "api.cloudflare.com"` num serve com `--jev-provider cloudflare` e chave
   sintética (nada é enviado; `the_status_reports_online_health_without_content` é o molde).
3. Doctor: `probe_from_env` passa provider e account id; a mensagem de falha de credencial
   (`credential.rs:24`) e o detalhe do `jev_probe` dizem "chave TypeSafe ou token Cloudflare,
   conforme `--jev-provider`".
4. Install: `.mcp.json` e o TOML carregam `--jev-provider P --jev-account-id ID` quando o provider
   não é o padrão (ou sempre que `--online`; escolher sempre explícito, é mais legível no arquivo).
   `KEY_NOTE` (`install.rs:374`) ganha a mesma frase da credencial.

### Ciclo 5 — log e redação

Arquivo: `tests/online_protocol.rs` (molde: `the_log_shows_each_call_sent_received_and_timed_without_the_key`, linha 921).

1. Teste `the_log_redacts_the_cloudflare_account_id`: cliente `loopback_with(.., Cloudflare, ..)`
   apontado a `http://127.0.0.1:P/client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef-flash`
   (o `loopback_with` monta o path da Cloudflare quando o provider é Cloudflare, com um account id
   fixo de teste), com `--log`. Asserções: o arquivo contém `accounts/[redacted]/` e não contém
   `abc123`. Red: hoje a URL sai inteira.
2. Teste de regressão `the_typesafe_log_is_byte_identical`: com `loopback` (TypeSafe), o bloco
   `enviado` continua `POST http://127.0.0.1:P/v1/systemone` e o `recebido` é o JSON como veio,
   sem re-serialização (comparar o trecho entre `HTTP 200\n\n` e `── duração` com `pretty(OK)`).
3. Green: `redact::endpoint_for_log` (substitui o segmento entre `/accounts/` e a próxima `/`);
   `jev.rs` usa ao montar `Exchange`. Moduledoc de `log.rs` registra a decisão 5. O `recebido`
   da Cloudflare loga o envelope inteiro como veio (é o que a rede trouxe; o desembrulho é
   depois do log).

### Ciclo 6 — regressão do padrão

1. Todos os testes existentes de `online_protocol.rs`, `online.rs`, `online_units.rs`,
   `online_no_key.rs` passam sem alteração de asserção (só os construtores, se a assinatura de
   `new` mudar). Este é o teste de "nada muda para quem não passa a flag".
2. `cargo test` sem a feature `online`: `cli.rs` e `install.rs` compilam com `JevProvider` fora do gate.

### Ciclo 7 — docs e PRD

1. README: linha da tabela de flags; "Online mode" com os dois provedores (endpoint, auth,
   modelos `clef`/`clef-flash`, envelope), limites (contexto 65.536 tokens para `clef` e
   24.576 para `clef-flash` segundo a página do modelo, contra `--jev-max-source-bytes` e os
   previews de 16/4/24 KiB: um lote de 16 candidatos a 16 KiB cabe em 65k mas não em 24k, dizer
   isso) e preços verificados (entrada 0,042 / 0,038 / 0,24 $/Mtok para jev / clef-flash / clef;
   saída da Cloudflare não cobrada), exemplo do log com `accounts/[redacted]/`.
   Corrigir a tabela do spec (`spec/prompt/jev-provider-cloudflare.md`): 0,09 → 0,038.
2. PRD `spec/ripwire-broker-mcp.md` §23.9 (linha 2234) e §23.12 (linha 2317): um parágrafo cada
   dizendo que a fronteira de provider (`Classifier`, `classifier.rs:1`) agora tem dois lados, e
   que a diferença é só de transporte (URL, token, envelope).
3. `spec/changelog.md`: `## D-166 — --jev-provider cloudflare` com as decisões 1–5 acima,
   incluindo a do account id no log.

### Ciclo 8 — live e entrega

1. `tests/online_live.rs`: `live_client()` lê `RIPWIRE_BROKER_LIVE_PROVIDER` e
   `RIPWIRE_BROKER_LIVE_ACCOUNT_ID` (padrão typesafe); modelo `clef-flash` quando cloudflare.
   Rodar `--ignored` com um token Cloudflare real, só corpus sintético (`jev_corpus`).
2. `ripwire-broker doctor --workspace . --jev-probe --jev-provider cloudflare --jev-account-id ID
   --jev-model clef-flash`: latência e `usage` do probe vão no PR; custo = `input_tokens × 0,038 / 1e6`.
   O probe também responde a dúvida do Content-Type: se falhar com `Malformed` e HTTP 200, a
   Cloudflare não mandou `application/json` e o `starts_with` de `jev.rs:244` precisa de um
   caso a mais (parar e explicar, é fora do diff cirúrgico).
3. PR: diff, saída de `cargo test --features online`, probe, decisão do account id.

## 3. Riscos e pontos de parada

- **`Copy` em `InvalidResponse`** (decisão 3): medir impacto antes do ciclo 2. Se for mudar
  `parse_answers` para acomodar, parar.
- **Doc verificada** (§0b): envelope, path e `usage` dentro de `result` confirmados; `CF_OK` do
  ciclo 1 segue essa forma. O que ficou sem fonte oficial: o Content-Type da resposta e a
  forma de cada item de `errors[]` (a convenção v4 da Cloudflare é `{code, message}`; o
  desembrulho serializa o array inteiro, então a forma não importa para o código).
- **Content-Type**: só o live test prova. Ver ciclo 8.
- **Contexto do clef-flash**: 24.576 na página do modelo, 64K no changelog. Documentar o menor.
- **Lote de perguntas (achado real)**: a Cloudflare aceita 64 por pedido; `request.rs:9` fecha
  um lote em **128** (`MAX_QUESTIONS`, exercitado em `tests/online_units.rs:317`), e a memória em
  32 (`memory/wire.rs:8`, segura). Com os padrões a descoberta manda no máximo
  `--jev-max-candidates 16 + --jev-lookahead-max 32 = 48` perguntas de admissão, dentro do
  limite; quem subir as duas flags passa de 64 e recebe 422 da Cloudflare. Baixar o cap em
  `request.rs` é mudar `request.rs`, que o spec proíbe. Opção cirúrgica, a decidir com o autor:
  `Flags::provider()` recusa `cloudflare` quando `max_candidates + lookahead_max > 64`, com
  usage que explica o limite. Teste de CLI no ciclo 3. Sem decisão, documentar o limite no
  README e deixar o 422 aparecer no log.
- **`MAX_RESPONSE_BYTES`** (256 KiB): o envelope acrescenta poucos bytes; sem mudança.
- **Cache**: a chave já inclui `provider` e `endpoint` (`cache.rs:15-16`); trocar de provider
  invalida naturalmente. Nada a fazer, mas confirmar com um teste se sobrar tempo.

## 4. Como foi implementado (2026-10-09) e onde saiu do plano

Ciclos 1 a 7 feitos, cada teste visto falhar antes do código; decisões em
`spec/changelog.md` D-166. Desvios do plano, todos por achado durante o TDD:

| Plano | Feito | Por quê |
|---|---|---|
| `result` re-serializado | `result` byte a byte (`RawValue`) | Re-serializar por `Value` colapsa chaves repetidas e esconde `DuplicateQuestion` do parser |
| Mudar a assinatura de `JevClient::new` | `new` intacto, mais `JevClient::for_provider` | Um teste fixa por tipo que o construtor não aceita URL; `for_provider` recebe o enum e o account id, e tem o mesmo teste |
| `endpoint()` devolve `String` | Devolve `Result`, e valida account id e modelo | A URL nunca é montada com o que ela não pode carregar; a CLI só repassa o erro |
| `redact::endpoint_for_log` só na linha do `POST` | O account id é um segundo segredo do bloco inteiro | Uma resposta que ecoa a URL vazaria o id |
| `install` grava as flags sempre | Só com Cloudflare | O snippet de quem usa TypeSafe continua byte a byte o de antes |
| 64 perguntas: só documentar | Teto de 64 no lote (`batches_within`), a pedido do usuário | Toca `request.rs` (onde o lote fecha, não a forma do pedido) |

**Ciclo 8 feito** (2026-10-09): probe e `online_live` contra a Cloudflare real, resultados em
`spec/changelog.md` D-166. O transporte funciona; o Content-Type é `application/json`. Um dos três
testes ao vivo reprova com o padrão `clef-flash` por julgamento do modelo (rejeita o arquivo de
teste do corpus), e passa com `clef`. A decisão 4 (padrão `clef-flash`) fica em aberto.
