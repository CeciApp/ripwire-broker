# Plano incremental — testes de propriedade e CI/CD

Data: **2026-10-04**. Base: workspace `fix/lows-online`, HEAD `dc56f0a`.
Especificação: [ci-cd.md](../prompt/ci-cd.md).
Status: **planejado; implementação não realizada nesta revisão documental**.

## Objetivo e limites

Fechar as lacunas da especificação revalidada, preservando o CI e as propriedades já
entregues. O plano histórico [proposta-pbt-e-ci-cd.md](proposta-pbt-e-ci-cd.md) está
cumprido; suas sete fatias não devem ser abertas novamente.

A entrega atual altera somente `spec/prompt/ci-cd.md` e cria este plano.
Os arquivos citados abaixo são alvos de uma implementação futura, não alterações
já autorizadas ou realizadas. Não inclui release, deploy, mudança de API pública,
upgrade de dependências, cache de build ou chamadas ao provider real.

## Base já implementada

- [x] `proptest`, testes puros/filesystem, orçamento via Broker e scheduler com fake.
- [x] `markup` e `budget` públicos; demais internos preservados.
- [x] Jobs `default`/`online`, fmt, clippy, nextest e isolamento do build default.
- [x] Actions por SHA, permissões mínimas, timeouts e checkout sem credenciais persistidas.
- [x] Supply chain separada de PR, deny.toml e Dependabot com política de pins.
- [x] Lints da biblioteca/binário principal, ignore de ambientes e guarda de fixtures.
- [x] Cobertura de propriedades de memória além do escopo P0/P1 original.

## Etapas de implementação

### 1. Persistência reproduzível de regressões — prioridade alta

**Arquivos:** `tests/props_fs.rs`, `tests/broker.rs`, `tests/online_scheduler.rs` e,
quando houver falha real reduzida, seus arquivos `tests/*.proptest-regressions`.

- [ ] Substituir `failure_persistence: None` por `FileFailurePersistence::Direct`
  com caminho explícito por alvo, seguindo `tests/props.rs`.
- [ ] Preservar 48 casos de filesystem, 32 de orçamento e 40 de scheduler.
- [ ] Garantir que a estratégia e o replay usam somente dados sintéticos e tempdirs;
  não persistir caminhos absolutos temporários como entrada gerada.
- [ ] Validar uma falha controlada/reduzida e seu replay em ambiente temporário,
  removendo a falha deliberada antes da entrega. Não versionar seed fabricada como
  se fosse regressão de produto nem modificar seeds existentes sem justificativa.

**Aceitação:** uma regressão legítima é reproduzida em nova execução pelo arquivo
correto; nenhuma propriedade escreve fora do tempdir ou precisa de rede.
**Dependências:** nenhuma. **Risco:** PBT com E/S pode produzir dados específicos da máquina;
inspecionar persistência e manter os limites por caso.

### 2. Oráculo de redação fiel ao contrato — prioridade alta

**Arquivos:** `tests/props.rs`; consultar `src/online/redact.rs`.

- [ ] Acrescentar casos dirigidos: segredo com forma imprimível vazia, Unicode removido,
  segredo curto, substring de `[redacted]`, múltiplas ocorrências e corte do marcador.
- [ ] Separar a garantia de saída ASCII/limite da substituição de ocorrências da forma
  imprimível do segredo. A asserção de ausência literal só é válida quando a forma
  não pode ser reintroduzida pelo marcador ou pelas fronteiras de substituição.
- [ ] Manter o caso que detectou vazamento do esqueleto ASCII. Usar resultados esperados
  explícitos nos casos dirigidos, sem copiar o algoritmo inteiro no oráculo.
- [ ] Se houver vazamento além da semântica documentada, produzir caso mínimo antes
  de propor correção em `src/online/redact.rs`; não transformar coincidência com
  marcador fixo em vulnerabilidade comprovada.

**Aceitação:** testes distinguem vazamento de entrada de coincidência com texto fixo;
passam nos dois conjuntos de features e preservam a cobertura Unicode.
**Dependências:** nenhuma; pode seguir a etapa 1 na mesma sequência de trabalho.

### 3. Guardas de CI falham de forma explícita — prioridade alta

**Arquivo:** `.github/workflows/rust.yml`.

- [ ] Executar `cargo tree --locked -e normal` em passo cujo erro interrompa o job,
  antes de procurar crates proibidos. Inspecionar nomes de pacote, alinhados ao
  teste CA-10 de `tests/mcp_surface.rs`, incluindo `h2` e `axum`.
- [ ] Na guarda de fixtures, distinguir códigos do grep: 0 significa ocorrência,
  1 significa ausência e erro de execução deve falhar. Não imprimir conteúdo de
  credencial; basta nome de arquivo e categoria.
- [ ] Validar cada guarda com entrada limpa, ocorrência proibida sintética e falha
  de inspeção, usando arquivos temporários e comandos substitutos isolados.
- [ ] Manter `default`/`online`, SHA pins, permissões, timeouts e ausência de segredos.

**Aceitação:** os três resultados são distinguíveis; erro de Cargo ou leitura não
produz mensagem de verificação bem-sucedida. A árvore default real passa e a online
serve como controle positivo do detector, sem bloquear o job online por ter HTTP.
**Dependências:** nenhuma. **Risco:** um detector amplo demais pode confundir nomes;
comparar pacotes completos em vez de substring de uma linha qualquer.

### 4. Execução de PBT compreensível e sem lacunas — prioridade média

**Arquivo:** `.github/workflows/rust.yml`, comentários/comandos de uso associados.

- [ ] Documentar que há propriedades nos alvos `props`, `props_fs`, `broker` e
  `online_scheduler`; a suíte completa já executa todos eles.
- [ ] Manter o passo de 4096 casos restrito a `binary(props)` e não aplicar essa
  carga aos alvos com E/S ou setup assíncrono.
- [ ] Disponibilizar comando focal para os quatro alvos ou para as duas propriedades
  nomeadas em Broker/Scheduler. Não criar nova execução redundante de toda a suíte.
- [ ] Preservar golden tests, testes de memória e helpers ignorados iniciados pelos
  testes pais. Não habilitar globalmente testes ignorados.

**Aceitação:** a seleção focal inclui P0.11/P1.3; execução default e online continua
completa. Os limites fixos permanecem verificáveis nos helpers de configuração.
**Dependências:** etapa 1, para registrar corretamente a persistência no uso documentado.

### 5. Validação de integração e configuração operacional

- [ ] `cargo fmt --all --check`.
- [ ] `cargo clippy --all-targets --locked -- -D warnings` e o mesmo com `--features online`.
- [ ] `cargo nextest run --locked` e `cargo nextest run --locked --features online`.
- [ ] `PROPTEST_CASES=4096 cargo nextest run --locked -E 'binary(props)'`.
- [ ] Executar a seleção focal abaixo, conferir replay e as guardas da etapa 3.
- [ ] Executar `cargo deny --all-features check` no contexto próprio de supply chain;
  registrar data e resultado. Achado novo de advisory não vira exceção automática.
- [ ] Consultar proteção/rulesets de `master` via acesso autenticado de leitura:
  verificar checks `default`/`online` e política equivalente ao `strict` registrado.
  Registrar evidência atual sem presumir que comentário do YAML configura o GitHub.
- [ ] Respeitar D-107 quanto a revisão humana. Se houver divergência remota, registrar
  o ajuste concreto necessário; não alterar política de merge implicitamente.
- [ ] Registrar a implementação futura em `spec/changelog.md` com o próximo D-NNN
  disponível, resultados reais e referência a este plano.

```sh
cargo nextest run --locked -E 'binary(props) | binary(props_fs) | (binary(broker) & test(the_budget_bookkeeping_stays_consistent_at_any_budget)) | (binary(online_scheduler) & test(the_scheduler_keeps_its_limits_and_never_mixes_up_an_answer))'
```

**Aceitação final:** etapas obrigatórias concluídas, duas configurações verdes,
seeds reproduzíveis e evidência operacional registrada. Uma indisponibilidade remota
é registrada como bloqueio da checagem operacional, nunca como aprovação presumida.
Práticas opcionais (cache, SBOM, harden-runner) não bloqueiam a conclusão.

## Verificação realizada na revisão documental

- `cargo metadata --offline --locked --no-deps --format-version 1`: sucesso;
  uma biblioteca, dois binários, dois exemplos e 23 alvos de integração;
  `rust_version = null`.
- `cargo tree --offline --locked -e normal`, default/online: sucesso;
  dependências HTTP opcionais ausentes no default e presentes com online.
- Regex atual de credenciais sobre fixtures: nenhuma correspondência; isso não
  atesta por si só a origem sintética de todo arquivo.
- `cargo test --offline --locked --test props --test props_fs`: sucesso;
  **44 testes em `props` e 10 em `props_fs`, nenhum falhou ou foi ignorado**.
  Resultado focal no build default, sem extrapolar para a suíte inteira ou online.

Não foram executados toda a suíte, nextest, clippy ou cargo-deny nesta revisão;
não houve consulta nem alteração de proteção no GitHub. As etapas acima são plano,
não alegações de trabalho concluído.
