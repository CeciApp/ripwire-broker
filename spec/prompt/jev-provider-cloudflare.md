# Implementar `--jev-provider cloudflare` no ripwire-broker

## Contexto

O broker fala com um classificador System One pelo protocolo `systemone` da TypeSafe
(`src/online/jev.rs`, `request.rs`, `response.rs`). Hoje `--jev-provider` aceita só
`typesafe`, e o endpoint é a constante `ENDPOINT` em `jev.rs`. A Cloudflare publicou o
Clef e o Clef-flash (01/10/2026), que implementam o mesmo protocolo de pedido e resposta
com três diferenças de transporte. Quero poder apontar o broker para eles sem tocar no
builder de pedidos nem no parser de respostas.

## Fatos sobre o Clef (verifique na doc antes de codar; não confie só nisto)

| Ponto | TypeSafe (atual) | Cloudflare |
|---|---|---|
| Endpoint | `https://api.typesafe.ai/v1/systemone` | `https://api.cloudflare.com/client/v4/accounts/<ACCOUNT_ID>/ai/run/@cf/cloudflare/<MODEL>` |
| Auth | `Authorization: Bearer <chave TypeSafe>` | `Authorization: Bearer <token Cloudflare com Workers AI Read+Edit>` |
| `model` no corpo | `jev-1.13.0` (pinado por `--jev-model`) | `clef` ou `clef-flash`; nome de Jev no corpo devolve erro 5006 |
| Corpo do pedido | `{model, state, questions}` | idêntico (mais `images`, que o broker não usa) |
| Resposta | `{model, answers, usage}` na raiz | envelope `{success, errors, messages, result: {model, answers, usage}}` |
| Contexto | — | clef 65.536 tokens; clef-flash 24.576 (página do modelo; o changelog diz 64K) |
| Preço de entrada | 0,042 $/Mtok | clef-flash 0,038; clef 0,24; saída não cobrada (página de preços, 2026-10-09; 0,09 era o do artigo de 01/10) |
| Perguntas por pedido | 128 | 64 |

Doc de referência: https://flaviocopes.com/clef/ e a doc de Workers AI da Cloudflare.

## O que implementar

1. **Flag `--jev-provider {typesafe|cloudflare}`**, padrão `typesafe`. Nada muda para
   quem não passa a flag.
2. **Flag `--jev-account-id <id>`**, obrigatória quando o provider é `cloudflare`, recusada
   quando é `typesafe`. Erro de CLI claro nos dois casos.
3. **Resolução do endpoint** no `build` de `JevClient`: o provider decide a URL. O
   `--jev-model` continua sendo o nome que vai no corpo e também compõe a URL da Cloudflare
   (`@cf/cloudflare/<model>`). Mantenha os construtores `loopback*` como estão; eles já
   provam que o endpoint é injetável.
4. **Desembrulhar o envelope** antes de `parse_answers`: se o JSON tiver `success` e
   `result`, use `result`; `success: false` vira `InvalidResponse` com os `errors`
   serializados na mensagem. A raiz sem envelope continua aceita. Essa decisão deve ficar
   no provider, não no parser: `parse_answers` não aprende o que é Cloudflare.
5. **Credencial**: mesma variável `RIPWIRE_BROKER_JEV_API_KEY`. Não crie uma segunda.
   Atualize o texto de ajuda e o `doctor` para dizer "chave TypeSafe ou token Cloudflare,
   conforme `--jev-provider`".
6. **`doctor --jev-probe`**, `memory drain --online` e `install ... --online` passam o
   provider e o account id adiante. O snippet que `install` grava em `.mcp.json` e no
   Codex tem que carregar as duas flags novas.
7. **Log (`--log`)**: a URL da Cloudflare contém o account id. Decida se ele é segredo;
   se for, redija como `accounts/[redacted]/` em `redact.rs`. Registre a decisão no
   moduledoc de `log.rs`.
8. **Status (`ripwire-broker://status`) e barra**: o segmento `[jev:N]` fica. Acrescente o
   provider ao `online` do status, só o enum.
9. **README**: tabela de flags, a seção "Online mode" com os dois provedores, limites
   (contexto de 65k contra `--jev-max-source-bytes`) e preços. PRD §23.9/§23.12 ganham
   um parágrafo dizendo que a fronteira de provider agora tem dois lados.

## Como implementar (ordem obrigatória)

- **Teste antes do código.** Suba um servidor loopback que responde com o envelope da
  Cloudflare e escreva o teste que falha hoje: o broker apontado a ele devolve
  `InvalidResponse` porque não acha `answers` na raiz. Confirme que falha por esse motivo.
  Só então implemente, e confirme que passa.
- **Controle negativo por asserção**: o mesmo teste com `success: false` e `errors`
  preenchidos tem que reprovar com a mensagem contendo o erro. Se passar sem o
  desembrulho, a asserção não mediu nada.
- Teste de CLI: `--jev-provider cloudflare` sem `--jev-account-id` falha; `--jev-provider
  typesafe --jev-account-id x` falha; a URL montada para `clef-flash` é exatamente a da
  tabela.
- Teste de regressão: com o padrão `typesafe`, a URL e o parsing são os de antes, byte a
  byte no log.
- O diff é cirúrgico: `request.rs` e `parse_answers` não mudam. Se parecer necessário mudar
  um dos dois, pare e explique antes.
- Rode `cargo test --features online` e os testes `online_live` ignorados com
  `RIPWIRE_BROKER_JEV_API_KEY` apontando a um token Cloudflare real, só conteúdo sintético.

## Fora de escopo

- Suporte a `images`.
- Servir Clef local (pesos abertos). Fica para depois; `loopback_without_key` já é o caminho.
- Mudar defaults de `--jev-deadline-ms` ou `--jev-max-candidates` por causa da latência menor.
  Meça primeiro com o probe e proponha em PR separado.

## Entrega

PR com: o diff, a saída de `cargo test`, uma rodada do `doctor --jev-probe` contra
`clef-flash` com latência e custo, e no corpo do PR a decisão sobre o account id no log.
