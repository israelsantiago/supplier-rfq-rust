# HTTP e OpenAPI: validação de contratos

`src/http/dto.rs` representa o protocolo HTTP externo e converte entradas para os requests internos das RPUs. Os testes verificam tanto as conversões quanto a desserialização JSON: campos opcionais, campos desconhecidos, valores de enum e datas ISO.

`docs/openapi.yaml` descreve endpoints, parâmetros, respostas e schemas. Execute localmente:

```bash
bash scripts/verify-openapi.sh
cargo test --lib dto_boundary_tests
cargo test --test acceptance_api
```

O CI possui um job separado para validar o OpenAPI. Se o lint revelar divergência entre a especificação e o comportamento HTTP efetivo, corrija a especificação ou o código conforme o contrato intencional e cubra o comportamento com um teste. Não remova regras de validação do OpenAPI apenas para fazer o lint passar.

## Escopo OpenAPI da idempotência

`Idempotency-Key` deve aparecer em exatamente cinco operações de comando: `openRfq`, `registerSupplier`, `setSupplierEligibility`, `submitSupplierQuote` e `closeRfq`. Não deve aparecer em `GET /api/rfqs/{rfq_id}/quotes`. `scripts/verify-idempotency-openapi.py` verifica esse contrato antes do lint Redocly.
