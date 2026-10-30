# Supplier RFQ — limites arquiteturais de capabilities e RPUs

Este documento registra os limites arquiteturais verificados entre Domain Capabilities, RPUs, Delivery Mechanisms e Application State. Ele não substitui as especificações oficiais de Rico Fritzsche.

## Estrutura das capabilities

Cada capability possui sua própria pasta de feature e um `process_request.rs` responsável por orquestrar a unidade de processamento. As operações de IO pertencem ao shell da própria capability. O core recebe dados explícitos, produz decisões determinísticas e deve ser testável sem banco, HTTP ou event store.

As queries podem ter `load_context.rs` e `build_context.rs`. Os comandos podem separar `load_context.rs`, `build_context.rs`, `generate_consequences.rs` e `append_consequences.rs` quando cada arquivo representa uma responsabilidade real. A estrutura não deve ser fragmentada mecanicamente.

## Fatos e eventos

`src/events/` é o módulo compartilhado de nomes versionados e contratos de payload de eventos/fatos. Regras de negócio, fluxo de comandos e lógica de projeção não pertencem a esse módulo. Os nomes dos eventos e os payloads persistidos são contratos duráveis; refatorações de organização devem preservá-los.

## Fronteiras

- A entrega HTTP desserializa DTOs HTTP e os converte para requests internos.
- Uma capability não importa internals de outra capability.
- A persistência de comandos deve preservar o contexto de consistência observado antes da decisão. Para FACTSTR, o append condicional continua condicionado à versão lida.
- Projeções de leitura são derivadas dos fatos. A aplicação de lotes deve permanecer atômica e replay/idempotência não podem regredir.
- Alterações arquiteturais devem ser acompanhadas por testes unitários, aceitação e concorrência relevantes.

## Verificação local

Execute:

```bash
bash scripts/verify-rpu-boundaries.sh
```

O workflow de CI executa o mesmo script, incluindo formatação, compilação, testes e Clippy.
