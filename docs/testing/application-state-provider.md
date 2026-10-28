# Testes do Application State Provider

## Contratos obrigatórios

O ASP é uma porta tecnológica, não um segundo núcleo de negócio. Ele expõe consultas nomeadas pelo domínio, fatos tipados, versão de contexto opaca e append condicional. O adaptador FACTSTR é dono da conversão para `EventQuery`/`EventRecord`/`NewEvent` e da tradução de conflito CCC.

| Contrato | Evidência esperada | Limite da evidência |
|---|---|---|
| Fato conhecido malformado falha fechado | `malformed_known_fact_fails_closed` | Testa desserialização no adaptador; não migra fatos históricos. |
| Tipo de fato desconhecido não é ignorado | `unknown_fact_type_fails_closed` | Operação precisa definir explicitamente novos fatos e queries antes de habilitá-los. |
| Versão ausente e presente fazem round-trip no adaptador | `version_token_preserves_empty_and_present_context` | O token só é interpretado pelo adaptador FACTSTR. |
| Leitura tipada, append e append obsoleto | `provider_reads_typed_facts_and_rejects_stale_conditional_append` | Usa PostgreSQL/Testcontainers; não é benchmark. |
| Consultas CCC mantêm o escopo original | testes `ccc_*` e `eligibility_history_does_not_change_assignment_context_version` | Prova os cenários implementados, não todas as combinações futuras de filtros/fatos. |
| Produção não importa FACTSTR nas capabilities | `scripts/verify-application-state-boundary.sh` | Guardrail estático; não substitui code review nem teste funcional. |

## Execução

```bash
cargo test --lib application_state::factstr::tests -- --nocapture
cargo test --lib ccc_ -- --nocapture
cargo test --test acceptance_api
bash scripts/verify-application-state-boundary.sh
cargo test --all-targets --all-features --locked
```

Os testes de integração precisam de Docker/Testcontainers. O gate de 100% de branches do Functional Core permanece limitado aos módulos listados no checker de cobertura; ele não mede o ASP, a fachada ou o middleware HTTP.
