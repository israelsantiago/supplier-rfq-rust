# ADR-0016: Testes unitários, propriedades e aceitação com PostgreSQL

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A correção funcional e as invariantes CCC precisam de evidências em diferentes níveis.

## Decisão

Usar testes unitários no Functional Core, proptest para propriedades matemáticas e Testcontainers/PostgreSQL para aceitação, replay e concorrência CCC.

## Consequências

Testes verdes demonstram somente cenários cobertos. Docker é necessário para os testes de aceitação baseados em containers; benchmarks e testes de falha operacional continuam sendo trabalhos distintos.

## Atualização — ASP e idempotência com falhas

A suíte deve testar cada camada com a evidência apropriada: Functional Core puro, adaptador ASP contra PostgreSQL real, concorrência CCC entre stores/conexões e estados/falhas do ledger HTTP. A meta de 100% de branches continua restrita aos arquivos explicitamente enumerados no checker. Os testes novos incluem claim concorrente, `in_progress`/`unknown`, falha ao persistir resposta, ledger indisponível antes do comando e ausência de duplicação de fato após falha pós-append. Testcontainers exige Docker.

## Política de infraestrutura Testcontainers: container compartilhado e banco isolado por teste

- Para executáveis de integração, um PostgreSQL 16 é compartilhado enquanto houver pelo menos uma fixture ativa. A thread dedicada mantém o container e seu runtime Tokio; o cache global guarda somente `Weak`, portanto, se todas as fixtures forem liberadas, o container é removido e um teste posterior pode iniciar um novo ciclo.
- Cada invocação do fixture cria um banco com nome único, derivado de UUID, dentro desse container. FACTSTR, a projeção e o ledger de idempotência de cada teste são inicializados nesse banco exclusivo.
- A criação de bancos é serializada pelo fixture para evitar conflitos no mecanismo PostgreSQL `CREATE DATABASE`/`template1`; os testes continuam livres para executar em paralelo após obterem seus bancos.
- O banco bootstrap do container não é compartilhado como banco de estado de teste. Os bancos descartáveis desaparecem quando o container efêmero é removido após a última fixture ativa; um ciclo posterior pode iniciar outro container se necessário. O cache global mantém apenas `Weak<SharedPostgres>`; cada `TestDatabase` retém uma referência `Arc` enquanto o teste está ativo. Ao cair a última referência, `Drop` sinaliza e aguarda a thread proprietária, que executa `container.rm().await` dentro do runtime Tokio. Não há FFI, `atexit` nem `unsafe`; a limpeza defensiva de sobras legadas identifica somente a imagem e as variáveis específicas do fixture.
- A política aplica-se somente a testes que usam Testcontainers. Testes puros do Functional Core, propriedades e testes que não usam Docker não devem importar esse fixture nem ganhar dependência de infraestrutura.
- A suíte deve ser executada com concorrência de testes habilitada (por exemplo, `--test-threads=8`) para validar isolamento e estabilidade do fixture; não normalizar execução serial como solução permanente.
- O guardrail `scripts/verify-testcontainers-isolation.sh` falha se os executáveis de integração voltarem a iniciar containers diretamente ou deixarem de incluir o fixture compartilhado.


## Separação entre Functional Core e testes de storage

Mutation testing e branch coverage do Functional Core não devem depender de Docker, PostgreSQL ou HTTP. Os runners excluem somente nove testes de storage existentes dentro do alvo library e usam `DOCKER_HOST` inválido como defesa fail-fast. Esses testes continuam obrigatórios na suíte completa. A decisão pura `FinalizeRfqSourcing` está incluída em coverage e mutation testing. `scripts/verify-functional-core-test-isolation.sh` mantém essa decisão verificável e falha fechado se aparecer um novo uso de Testcontainers não catalogado.

<!-- ADR-0016-WIREMOCK-RS:START -->

## Atualização — contratos HTTP externos com WireMock-RS e Testcontainers

Para testar integrações HTTP de Providers externos da Application, usar o crate Rust `wiremock` 0.6.5 (`wiremock-rs`) como servidor mock. Como o crate executa `MockServer` dentro do processo e não oferece distribuição standalone, o repositório mantém um binário test-only em `tests/wiremock-rs/`, empacotado numa imagem própria e iniciado por Testcontainers Rust (`GenericImage`). A imagem não é `wiremock/wiremock` (WireMock Java) e não é artefato de produção.

- `tests/contracts/external-providers-v1.openapi.yaml` é a fonte do contrato HTTP provisório do test double.
- `tests/external_provider_contracts.rs` é teste integrado e exige Docker e imagem local criada por `scripts/build-amri-wiremock-rs-image.sh`.
- O test double cobre 200 tipado, 204 NotApplicable, 404 NotFound, 503 Unavailable, 504 TimedOut e payload inválido.
- Testes unitários dos ports/Fakes não iniciam containers; `cargo test --lib --all-features --locked app::amri::providers::tests -- --test-threads=1` executa os testes unitários dos Providers/Fakes sem Docker. O alvo `lib` contém testes legados de storage com Testcontainers; para demonstrar isolamento, usar o filtro específico e não afirmar que a suíte `--lib` inteira é livre de Docker.
- Functional Core, AMRI Core e Prepared Participants não importam `wiremock`, Testcontainers ou clientes HTTP.
- Mutation testing/branch coverage do Functional Core continuam restritos à lógica pura. O novo teste integrado não entra nesses runners.
- Não usar `sleep` para simular timeout. Usar resposta 504 para contrato de gateway e falhas determinísticas; timeout de socket deve ser testado pelo adapter HTTP quando existir.

A existência do mock não prova interoperabilidade com sistemas reais. O OpenAPI é contrato do harness do projeto, não contrato oficial de qualquer fornecedor.
<!-- ADR-0016-WIREMOCK-RS:END -->
