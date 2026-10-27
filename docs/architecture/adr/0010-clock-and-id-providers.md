# ADR-0010: Providers explícitos para relógio e IDs

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A shell depende de relógio e aleatoriedade; o núcleo funcional precisa continuar puro e os efeitos devem ser substituíveis.

## Decisão

Definir `ClockProvider` e `IdProvider`, implementar providers de produção e compô-los em `main.rs`/`ApiState`. A shell obtém valores concretos e passa-os às decisões puras.

## Consequências

Facilita testes determinísticos e explicita as dependências. Os Providers não entram nos DTOs nem são chamados dentro do Functional Core.
