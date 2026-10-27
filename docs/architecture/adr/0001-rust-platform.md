# ADR-0001: Rust 2024 e plataforma de compilação

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

O projeto precisa de uma linguagem com segurança de memória, tipos fortes e ecossistema adequado a serviços de backend.

## Decisão

Usar Rust Edition 2024, `rust-version = 1.85` e Cargo como build/dependency manager. Fixar versões compatíveis no `Cargo.lock`.

## Consequências

A compilação e os testes devem ser verificados nas versões suportadas; atualizações de toolchain e dependências são mudanças revisáveis.
