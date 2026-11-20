# Stage 5B.2 — reconciliação mínima de evidências

**Natureza:** pacote temporário, evidence-only. Não altera código nem promove status da fase.

**Arquivos selecionados:** 7
**Raiz examinada:** `DATA_DOWNLOADS` (/mnt/Dados/MeusDados/Downloads)
**`~/Downloads`:** não examinada; cópia espelhada ignorada.

## Categorias não encontradas
- Nenhuma categoria ausente pelos critérios de nome/conteúdo do coletor.

## Arquivos publicados

| Categoria | Caminho publicado | Redações | SHA-256 publicado |
|---|---|---:|---|
| receipt, apply, testcontainers, provider_integration, boundary_openapi_docs | `artifacts/apply/apply-receipt.json` | 5 | `d4ce5a95adc7229cade0e828dd41bc04f6a463ea3ea08b9874c15017124c3d85` |
| receipt, apply, verify, testcontainers, provider_integration, boundary_openapi_docs | `artifacts/verify/verify-receipt.json` | 20 | `52cb80cfe82c21ef3b78f7008ee48760d4cb80e0dcb91447c89ac03f655fc0f3` |
| apply, verify, testcontainers | `artifacts/standalone/amri-stage5b2-verify-repaired-20261117T223103Z.log` | 19 | `c83638083952974745d32f241b5cc304d70b48bedee33113dec7d7bb67df94e6` |
| apply, wiremock_build, testcontainers, provider_integration | `artifacts/standalone/amri-stage5b2-wiremock-build-final-20261118T171744Z.log` | 13 | `3fc455e89727964bb5831ccd4e2beae32aff08410d2801e5e861afedebee75e3` |
| testcontainers, provider_integration | `artifacts/standalone/amri-stage5b2-testcontainers-trace.log` | 2 | `9323dff3f86afb449843b7dc21c3ca096cb6175088c07c5a7fdeb4db8af6cf8e` |
| testcontainers, provider_integration, boundary_openapi_docs | `artifacts/verify/logs/testcontainers_isolation.log` | 0 | `33936bd498f4d70dbdc54510ede01842c30da19a2ebfb2781b2fb49ff781371c` |
| boundary_openapi_docs | `artifacts/verify/logs/documentation.log` | 0 | `2f32289daea51393221c370133372d6b5110d0425ffdb8290337b2947995ab65` |

## Limitações

- O coletor seleciona recibos nos diretórios Apply/Verify mais recentes e, no máximo, um log representativo por categoria.
- Arquivos antigos, backups, pacotes TAR/ZIP, diffs, scripts, caches e a árvore espelhada HOME_DOWNLOADS foram excluídos.
- A ausência de uma categoria significa que o coletor não encontrou evidência elegível; não significa que o gate falhou.
- Nenhum resultado `PASS` é inferido apenas pelo nome do arquivo.
- `EVIDENCE-MANIFEST.json` registra hashes da origem e do conteúdo publicado após sanitização.
