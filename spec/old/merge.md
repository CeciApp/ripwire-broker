Tarefa: fundir o adaptador `--online` no PRD pai do ripwire-broker.
Não invente arquivo, seção ou versão. Se um path abaixo não existir, pare e reporte.

## Fontes (pinadas)

- PRD pai (editar uma cópia, nunca o attachment original):
  `/spec/ripwire-broker-prd.md`
- Spec do adaptador a transportar (única fonte Jev):
  `/spec/jev-integration-prd.md`  (v0.2.1)

## Saída

- Escrever `/spec/ripwire-broker-mcp.md`
- Bump da versão do PRD para 0.3
- Não apagar os arquivos Jev neste turno; o merge deve tornar essa deleção possível depois, sem perda de requisito normativo

## Roadmap

Os blocos `### Fase 0`, `### Fase 1`, `### Fase 2` e `### Fase 3` devem permanecer
byte-a-byte idênticos ao pai (incluindo “Estado: implementada” já existente).

- Renomear o bloco atual `### Fase 4 — Times e CI` para `### Fase 6 — Times e CI`
- O conteúdo desse bloco permanece o mesmo, salvo atualizar referências internas
  que apontavam “Fase 4” para Times/CI (ex.: §21.4, runner de CI) para “Fase 6”
- Inserir duas fases novas, derivadas só da spec v0.2.1:

  Fase 4 — Adaptador `--online` mínimo (shippable atrás da flag, sem lookahead):
  Sprint 0 (`RankedPath` antes do budgeter + fixture live `jev-1.13.0` + `prompts/v1`);
  feature/flag/credencial só via env do MCP; cliente TypeSafe pinado; dois `noul`;
  gate por rota + `#ripwire-off`; rescore dos paths do planner; merge aditivo;
  cache memória; teto 4 in-flight / 24 requests; barra de merge de engenharia
  que não dependa de lookahead.

  Fase 5 — Completar `--online`:
  lookahead de um nível; revalidação de hash; retry/429/cancel até o HTTP;
  redaction; `doctor --jev-probe`; skill/install/`env`; métricas; corpus A/B e
  barra de produto. Sem fronteira remota de diretórios.

Não criar tool MCP nova. Não ligar Jev em `context_after_edit` / `context_before_finish`.

## Transporte

Tudo que for requisito, invariante, schema aditivo, teto, prompt `v1`, CA,
risco ou decisão em aberto na spec v0.2.1 deve aparecer no arquivo de saída
(capítulo próprio, p.ex. §23), de modo que apagar o avulso Jev não perca norma.

Não transportar: engenharia reversa do jevgrep, tabela comparativa longa,
números SWE-bench de terceiro, pipeline de 8 estágios, 32 in-flight, hard stop
50.000, papéis Jev, passagem relacional.

## jevgrep

No máximo uma menção no documento, e só para dizer que é inspiração — não
dependência, não implementação de referência, não KPI. Preferir a expressão
“classificador semântico remoto”.

## O que pode mudar fora do Roadmap

Permitido: sumário, §6.4–6.6 (encolher comparação), caixa opcional no diagrama
do §7 claramente marcada “somente --online”, nota aditiva no envelope v1,
RF ponte, riscos, §21 novo item, referências TypeSafe.

Proibido: reescrever Fases 0–3; afrouxar CA-10 / offline default; transformar
probabilidade em caller/teste; usar `jev-latest` como default.

## Verificação obrigatória no final da resposta

1. Diff dos quatro blocos `### Fase 0` … `### Fase 3` contra o pai: idênticos.
2. Existe `### Fase 6 — Times e CI` e não existe mais `### Fase 4 — Times e CI`.
3. Contagem de “jevgrep” ≤ 1.
4. Lista do que a spec v0.2.1 tinha como RF/CA/teto/prompt e onde isso ficou
   no arquivo de saída.
5. Declaração explícita se o avulso Jev já pode ser removido sem perda, ou o
   que ainda falta.