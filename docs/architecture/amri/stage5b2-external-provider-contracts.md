# Stage 5B.2 — Contratos externos read-only e WireMock-RS

**Estado:** contratos provisórios de teste e portas Application propostos; nenhum endpoint de fornecedor real foi integrado. Aplicação e gates no checkout do mantenedor ainda pendentes.

## 1. Objetivo e limites

A Stage 5B.2 define contratos de portas tipadas para três fontes externas: ERP Reference, Compliance e Tax. O escopo implementado nesta entrega é limitado a:

- portas síncronas e read-only na Application;
- resultados de lookup explicitamente tipados;
- metadados mínimos de proveniência;
- Fakes determinísticos para testes sem infraestrutura;
- contratos HTTP versionados usados somente pelo test double;
- testes integrados HTTP contra uma instância WireMock escrita em Rust e iniciada por Testcontainers.

Não há URLs, credenciais, SDKs ou endpoints de fornecedores reais. O OpenAPI em `tests/contracts/external-providers-v1.openapi.yaml` é um contrato de referência **proprietário do projeto e restrito ao harness de testes**; não deve ser apresentado como especificação oficial de qualquer ERP, bureau de compliance ou serviço fiscal.

A auditoria do domínio não identificou atributos fiscais, verdictos de compliance externo ou identificadores de ERP que possam ser usados com segurança em decisões RFQ existentes. Por isso os três contratos retornam apenas referências externas e proveniência. Não introduzem `tax_code`, `tax_rate`, base de cálculo, `compliance_status`, `risk_score`, regras de homologação, códigos de organização de compras ou identificadores de produto não existentes.

## 2. Portas e resultados

As interfaces ficam sob `src/app/amri/providers/` e são `pub(crate)`. Uma fachada `provider_test_support`, compilada somente com `test-support`, reexporta tipos/fakes para os testes de integração, sem ampliar a API normal de produção.

| Porta | Pergunta provisória do contrato de teste | Resposta tipada |
|---|---|---|
| `ErpReferenceProvider` | Existe referência ERP associada ao `supplier_id` conhecido? | `ErpSupplierReferenceEvidence` |
| `ComplianceProvider` | Existe referência externa de evidência de compliance associada ao `supplier_id`? | `ComplianceEvidenceReference` |
| `TaxProvider` | Existe referência externa de evidência fiscal associada ao `supplier_id`? | `TaxEvidenceReference` |

Os três DTOs incluem `supplier_id`, uma referência opaca, `source_revision` opcional, `observed_at` e `expires_at` opcional. Compliance não retorna aprovação/reprovação. Tax não calcula nem retorna tributos. ERP Reference não realiza criação/atualização de cadastro.

O resultado de lookup distingue:

- `Found(T)`: resposta encontrada e decodificada;
- `NotFound`: recurso ausente na fonte;
- `NotApplicable`: a consulta não se aplica;
- `Unavailable`: fonte indisponível ou falha operacional 5xx/429;
- `TimedOut`: timeout de transporte/gateway;
- `InvalidResponse`: status/payload incompatível com o contrato.

`expires_at` é metadado fornecido pela fonte. `ProviderEvidenceMetadata::freshness_at(instant)` classifica somente a expiração explicitamente informada: `Fresh` antes do instante de expiração, `Stale` a partir dele e `UnboundedBySource` quando a fonte não informa expiração. Não há max-age inferido nem rejeição automática: o consumidor futuro decide o que fazer com evidência expirada.

## 3. Semântica HTTP do contrato de teste

| HTTP | Significado | Resultado Application |
|---:|---|---|
| 200 | Evidência tipada encontrada | `Found(T)` após decodificação válida |
| 204 | Consulta não aplicável | `NotApplicable` |
| 404 | Referência/evidência inexistente | `NotFound` |
| 503 / 429 / demais 5xx | Fonte indisponível | `Unavailable` |
| 504 | Gateway timeout | `TimedOut` |
| outro status/payload 2xx inválido | Contrato remoto incompatível | `InvalidResponse` |

Timeouts de socket/cliente também devem ser mapeados para `TimedOut` por um eventual adapter HTTP real. Esse adapter não integra a Stage 5B.2 porque não há endpoints reais aprovados.

## 4. WireMock-RS dentro de Testcontainers

O projeto usa `wiremock` crate 0.6.5 (`wiremock-rs`), implementação em Rust. Ela fornece um `MockServer` embutido no processo e não é uma distribuição standalone compatível com a imagem oficial `wiremock/wiremock` (Java). A documentação do projeto Rust também informa que não há modo standalone no crate; a documentação de WireMock indica que o módulo Testcontainers específico para Rust ainda não foi publicado.

Para executar exatamente a implementação Rust dentro de um container, `tests/wiremock-rs/` fornece um pequeno binário de teste que inicia `wiremock::MockServer` em `0.0.0.0:8080`. `tests/wiremock-rs/Dockerfile` empacota esse binário numa imagem própria, `supplier-rfq-amri-wiremock-rs:0.6.5-stage5b2-v1`. O teste `tests/external_provider_contracts.rs` inicia a imagem por `GenericImage` do Testcontainers Rust e aguarda o marcador `AMRI_WIREMOCK_RS_READY`.

A imagem contém somente o servidor mock de testes. A tag não é uma imagem publicada pelo fornecedor WireMock e não deve ser promovida a runtime de produção. O build local é executado por `scripts/build-amri-wiremock-rs-image.sh`; CI precisa executar o mesmo build antes do alvo integrado.

O servidor fornece respostas de sucesso tipadas e cenários fixos para 204, 404 (recurso ausente), 503, 504, 400 e JSON malformado. Não usa `sleep`, rede externa ou variáveis de ambiente secretas. O teste confirma que as respostas HTTP e a taxonomia de erro mantêm `NotFound`/`NotApplicable` distintos de `Unavailable`/`TimedOut`.

## 5. Isolamento de testes

- Testes unitários das portas/Fakes usam somente valores em memória. Não iniciam WireMock, Docker, PostgreSQL ou Testcontainers.
- Testes do AMRI Core (`src/amri/`) e Functional Core (`src/domain/`) não importam Providers, `wiremock`, `reqwest` ou Testcontainers.
- A suíte `tests/external_provider_contracts.rs` é a única consumidora do container WireMock-RS e pertence a testes integrados, não à cobertura/mutation do Functional Core.
- A suíte PostgreSQL permanece sob seu fixture próprio `tests/common/postgres.rs`; não compartilhar o container de WireMock com PostgreSQL.
- `cargo test --lib --all-features --locked app::amri::providers::tests -- --test-threads=1` executa somente os testes unitários dos Providers/Fakes sem Docker. O alvo `lib` possui testes históricos de storage com Testcontainers; a suíte `--lib` inteira não é livre de Docker.
- `cargo test --test external_provider_contracts --all-features --locked` exige Docker e a imagem local construída pelo script.

## 6. Fronteiras de arquitetura

Providers entregam evidência; não decidem política de domínio, não chamam participantes, não acessam Application State/FACTSTR e não fazem writes. Os Providers não entram em `src/amri/` nem em `src/domain/`. A Stage 5B.2 não integra os Providers ao `ExecutionContext`, não cria Reactor, não chama esses Providers a partir de participantes e não implementa policies cross-capability.

A precedência e os contratos das RPUs autônomas, incluindo `FinalizeRfqSourcing`, permanecem inalterados. Não existe compromisso distribuído entre os serviços externos e FACTSTR.

## 7. Referências técnicas

- [wiremock-rs — crate e README](https://github.com/LukeMathWalker/wiremock-rs)
- [wiremock 0.6.5 — MockServer](https://docs.rs/wiremock/0.6.5/wiremock/struct.MockServer.html)
- [Testcontainers Rust 0.27.3 — crate/API](https://docs.rs/crate/testcontainers/0.27.3)
- [WireMock — Testcontainers](https://wiremock.org/docs/solutions/testcontainers/)
