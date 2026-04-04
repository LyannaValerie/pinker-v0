# Bloco 04 — bare metal / kernel

## Status
Encerrado.

## Tese
Empurrar a Pinker para um primeiro recorte de linguagem voltada a bare metal sem prometer ambiente operacional rico.

## Dependências
Blocos 01 a 03 consolidados.

## Escada interna
- inline asm;
- freestanding / no-std;
- linker script / boot entry;
- primeiro kernel mínimo.

## Estado factual atual
O bloco segue fechado como recorte histórico pequeno e auditável de aproximação a bare metal textual, sem competir com as trilhas posteriores de backend, tooling ou superfície documental.

## Limites explícitos
- não implica kernel robusto;
- não implica runtime bare-metal amplo;
- não implica backend nativo pleno.

## Relação com o histórico
- Base factual antiga preservada em `docs/history/phases/001a050.md` e `docs/history/phases/051a100.md`.
- Continuidade documental posterior preservada em `docs/history/documentation/001a050.md`.
