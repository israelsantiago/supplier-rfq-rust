# ADR-0004: Functional Core / Imperative Shell

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Regras de negócio precisam ser previsíveis, testáveis e separadas de I/O e fontes não determinísticas.

## Decisão

A shell lê o estado e executa efeitos. O núcleo funcional recebe fatos/contexto, instante e IDs concretos como valores e produz uma decisão sem I/O.

## Consequências

O núcleo não acessa banco, relógio, aleatoriedade, rede nem estado global. Testes puros não precisam de PostgreSQL ou mocks de infraestrutura.
