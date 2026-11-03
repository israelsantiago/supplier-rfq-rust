# Estado atual do AMRI

**Atualizado em:** 2026-11-03\
**Branch:** `feat/amri-stage4-three-prepared-participants`\
**Repositório:** `israelsantiago/supplier-rfq-rust`\
**Diretório local usual:** `/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust`

## Commit de referência Stage 4A verificado

O GitHub foi consultado diretamente nesta sessão. O commit publicado da Stage 4A é:

- **HEAD publicado Stage 4A:** `203589e4d2c23338c8040e7802fabc77203fd6a9`
- **Commit pai:** `7b004771cc1f42ab5def3cbda1c62bff50a92ff2`
- **Mensagem:** `feat(amri): extract shared Functional Core for Stage 4A`
- **Comparação:** 1 commit à frente do pai e 0 atrás.
- **Arquivos Stage 4A:** exatamente 7 (3 novos em `src/domain/`, 3 arquivos de capability alterados e `src/lib.rs`).
- **URL:** [commit Stage 4A](https://github.com/israelsantiago/supplier-rfq-rust/commit/203589e4d2c23338c8040e7802fabc77203fd6a9).

A consulta remota confirmou que a branch existe, é pública e contém esse commit como último commit da implementação Stage 4A naquele momento. O pacote documental agora será instalado e publicado na mesma branch, portanto o HEAD da branch avançará para um commit documental. Registre esse novo SHA no `stage-log.md` e neste arquivo depois do push documental. A informação de `working tree` local precisa ser verificada no início de cada execução: não é possível inferir limpeza local a partir do GitHub.

## Matriz de status

| Fase | Estado | Evidência/status conhecido |
|---|---|---|
| 0 — Baseline e proteção | Concluída anteriormente | Inventário e gates registrados em `baseline.md` e plano original. |
| 1 — ADR e protocolo | Concluída como design | ADR-0034 e documentos AMRI; ADR-0034 permanece Proposed. |
| 2 — AMRI Core puro | Implementada e verificada anteriormente | `src/amri/`; testes puros, cobertura/mutação scoped e verificador de fronteira descritos no plano original. |
| 3 — Composite Context + state port | Implementada e verificada anteriormente | Planner, snapshot/token opaco e adaptador FACTSTR 0.5.2; evidências descritas no plano e Stage 3 doc. |
| 4A — Shared Functional Core | Implementada, verificada pelo mantenedor e publicada | Commit `203589e4...`; decisão compartilhada de lifecycle/sourcing. |
| 4B — Prepared Participants | **Não aplicada; V2 preparada, dry-run real pendente** | A V1 falhou por formatação/resolução de módulos nos temporários. A V2 passou nos testes Python/fixture do patcher, mas seu dry-run real no checkout ainda aguarda a publicação deste checkpoint documental. Nenhuma implementação 4B foi aplicada. |
| 5 — Reactor + Providers | Pendente | Ainda não iniciar. |
| 6 — Composition Policies + Commit Plan | Pendente | Ainda não iniciar. |
| 7 — Commit atômico, marker e recovery | Pendente | Ainda não iniciar. |
| 8 — Hardening operacional | Pendente | Ainda não iniciar. |
| 9 — Generalização | Pendente | Ainda não iniciar. |

**Importante:** o mantenedor informou que a aplicação e a verificação completa da correção Stage 4A V2 terminaram com sucesso. Como a saída integral desses comandos não está anexada a este documento, registre-a como `Reported by maintainer`; o commit remoto comprova a publicação dos arquivos, não substitui o log local de testes.

## Bloqueio atual da Stage 4B V1

Comando executado:

```bash
bash "$TOOLS/apply-stage4b-prepared-participants-v1.sh" "$REPO" --dry-run
```

Resultado: `[ABORT] rustfmt --check recusou o conteudo gerado; nenhum arquivo foi alterado`.

As diferenças de formatação eram corrigíveis pelo rustfmt. Separadamente, o rustfmt reportou que não encontrou `amri.rs` e `rfq_sourcing.rs` ao tentar resolver módulos filhos a partir dos arquivos temporários. A causa é o uso de arquivos Rust avulsos sem impedir a resolução de módulos filhos; a V1 também só verificava, não formatava previamente os conteúdos gerados.

A correção Stage 4B V2 deve evitar opções instáveis do rustfmt e não deve passar `src/lib.rs` isolado ao formatter, pois ele declara a árvore de módulos completa do crate. O desenho corrigido:

1. Preservar/copiar `rustfmt.toml` ou `.rustfmt.toml` da raiz, se existir.
2. Formatar `rfq_sourcing.rs` em arquivo temporário isolado com `rustfmt --edition 2024` (ele contém testes inline e não declara módulos externos).
3. Ler o conteúdo formatado de volta para o diff e para a aplicação.
4. Executar `rustfmt --edition 2024 --check` no mesmo arquivo temporário.
5. Validar estruturalmente as adições de `src/lib.rs` e `amri_participants/mod.rs`, cujas declarações são triviais e exatas.
6. Depois de `--apply`, executar `cargo fmt --all -- --check` no crate real, que valida a árvore completa.

A opção rustfmt `skip_children` não será usada, pois é instável e pode depender de toolchain nightly. Qualquer erro de formatação aborta antes de modificar o repositório.

O patcher V2 foi exercitado com seis testes Python simulados neste empacotamento; esses testes não executam o binário real do rustfmt. Este ambiente de geração não possui `rustfmt`, `cargo` ou `rustc`. Portanto, o dry-run real e os gates Cargo continuam pendentes no Ubuntu do mantenedor, sem qualquer afirmação de compilação feita aqui.
## Próximos passos obrigatórios

1. Aplicar/commit/push este pacote documental na branch atual, depois de revisar o `dry-run` documental. Isso cria um ponto de continuidade independente da implementação 4B.
2. Rodar os testes do patcher Stage 4B V2.
3. Rodar o `dry-run` Stage 4B V2 revisado. Confirmar branch/HEAD remoto, allowlist, âncoras Stage 4A, ausência de infraestrutura e diff formatado.
4. Revisar especialmente a exposição pública experimental de `crate::domain` e `crate::amri_participants`.
5. Só então executar `--apply` e o verificador Rust no Ubuntu.
6. Revisar diff, criar commit e push fast-forward da Stage 4B em operações separadas.
7. Depois do push, consultar o commit remoto e atualizar os documentos antes de iniciar Stage 5.

## Riscos e decisões em aberto

- A promoção de `pub(crate) mod domain;` para `pub mod domain;` é necessária no desenho V1 para disponibilizar tipos usados em assinaturas públicas de participantes. Isso aumenta a API pública e deve continuar explicitamente experimental até revisão. Não esconder warnings com `#[allow(dead_code)]`.
- O contrato `SourcingContext` é uma projeção tipada ampla compartilhada por Award e pelo `FinalizeRfqSourcing` legado. Revisar se a visibilidade pública integral é desejada ou se uma fachada pública experimental deve limitar melhor a superfície.
- Close e Award não têm dependência entre si. Ambos devem consumir projeções do mesmo snapshot único adquirido pela Stage 5/porta Stage 3.
- O resultado de Close não pode ser usado para alterar artificialmente `SourcingContext.closed` antes de Award; as duas decisões observam o estado anterior e as duas consequências são propostas no lote.
- `AwardedSupplierQuote` é o hand-off validado que preserva award fact e quote original/itens; não substituí-lo pelo `RfqAwardedFact`, que não contém todos os detalhes de linha.

## Comandos de verificação inicial ao retomar

```bash
REPO="/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust"
BRANCH="feat/amri-stage4-three-prepared-participants"

git -C "$REPO" status --short
git -C "$REPO" branch --show-current
git -C "$REPO" rev-parse HEAD
git -C "$REPO" ls-remote --heads origin "refs/heads/$BRANCH"
```

O HEAD remoto deve coincidir com o HEAD local antes de aplicar a próxima mudança. Após o checkpoint documental, o HEAD esperado é o novo commit `docs(amri): capture project continuity and phase agreements`; seu SHA deve ser anotado aqui após o push. Não execute novamente `publish-stage4a.sh`: aquele publisher espera o baseline anterior à Stage 4A e já não é válido depois do commit `203589e4...`.

## Ocorrência documental — falso positivo do instalador V1

Em 2026-11-03, o `apply-amri-continuity-docs-v1.sh --dry-run` abortou com `arquivos de continuidade ja existem com conteudo divergente` para `docs/architecture/amri/README.md` e `implementation-plan.md`. A inspeção do instalador confirmou falso positivo: `build_payload` gera append dos marcadores nos dois documentos existentes, mas a verificação de colisão os comparava com o conteúdo antigo como se qualquer diferença fosse sobrescrita indevida. O comando abortou antes da escrita; os dois documentos e a árvore do repositório permaneceram inalterados. A V2 do instalador permite somente o append determinístico preservando o conteúdo anterior, exige exatamente um par de marcadores e continua recusando divergência de conteúdo já gerenciado. Testes adicionais cobrem append, idempotência e marker drift.

## Estado da correção Stage 4B V2

O patcher Stage 4B V2 foi preparado e os seis testes automatizados do pacote passaram, incluindo a formatação simulada em fluxo `format-then-check` sem opções rustfmt instáveis. Isso comprova apenas o patcher/fixtures; não é evidência de que o binário rustfmt real, Cargo, Clippy ou os testes Rust passaram no checkout. O dry-run real Stage 4B V2 deve ocorrer somente depois da instalação e publicação do checkpoint documental V2 descrito no README do pacote.
