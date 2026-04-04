# Bloco 02 — memória explícita

## Status
Encerrado.

## Tese
Introduzir a camada de modelagem explícita de memória necessária para a Pinker deixar de ser apenas uma linguagem escalar.

## Dependências
Bloco 01 consolidado.

## Escada interna
- ponteiros;
- acesso a campo e indexação;
- casts controlados;
- `sizeof` / alinhamento;
- `volatile`.

## Estado factual atual
O bloco permanece como fundação histórica de tipagem e modelagem de memória, posteriormente operacionalizada apenas em blocos posteriores mais específicos.

## Limites explícitos
- não equivale à memória operacional completa em runtime/backend;
- não equivale a backend nativo real;
- não equivale a kernel ou tooling.

## Relação com o histórico
- Base factual antiga preservada em `docs/history/phases/001a050.md`.
- Continuidade macro da trilha antiga permanece referenciada em `docs/history/documentation/001a050.md`.
