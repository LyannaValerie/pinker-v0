# Handoff da Doc-46 — bare-metal e bootstrap

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## Resultado

A trilha bare-metal e bootstrap foi formalizada documentalmente no Bloco 20, sem mudança funcional e sem declarar suporte freestanding implementado.

## Estado preservado

- Fase funcional mais recente: 240.
- Bloco ativo: 20.
- Backend nativo operacional atual: ELF Linux x86-64 System V + `pinker_rt`.
- Eixo B: encerrado na Fase 222.

## Decisão registrada

A sequência para SO passa a distinguir:

1. capacidades da linguagem no Eixo A;
2. paridade nativa Linux no Eixo B;
3. Faixa 3 para ponteiros de função, alocador e inline assembly;
4. trilha BM1–BM9 para target freestanding, objeto, linker, entrada, runtime sem host, boot, imagem, QEMU e CI;
5. Faixa 7 para memória/layout/ABI;
6. Faixas 10–11 para kernel, concorrência, dispositivos e rede.

## Limite

Este handoff não substitui `docs/handoff_codex.md`; acompanha o PR da Doc-46 para revisão e deve ser incorporado ao handoff canônico antes do merge, caso a organização documental seja aceita.