# Trilha bare-metal e bootstrap do Bloco 20

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

## Objetivo

Fechar a lacuna entre o backend nativo Linux já existente e o primeiro artefato Pinker executável sem sistema hospedeiro.

Esta trilha não declara suporte bare-metal implementado. Ela organiza dependências, critérios de pronto e marcos verificáveis para que as capacidades do Eixo A possam convergir em um kernel real.

## Posição no Bloco 20

A trilha entra depois da Faixa 3 e antes da consolidação de baixo nível da Faixa 7:

```text
Faixa 1 — abstrações de linguagem
  -> Eixo B — paridade nativa Linux
  -> Faixa 3 — ponteiros de função, alocador e inline assembly
  -> trilha bare-metal e bootstrap
  -> Faixa 7 — memória, layout, ABI e atomics
  -> Faixas 10–11 — kernel, concorrência, dispositivos e rede
```

Ela é uma ponte de toolchain e execução, não uma nova promessa de funcionalidade concluída.

## Estado já disponível

- backend próprio `.s` x86-64 System V;
- geração de ELF Linux por `pink build --nativo`;
- runtime nativo `pinker_rt` ligado por ABI C;
- ABI de funções, controle de fluxo, texto, coleções, leques e famílias sistêmicas no recorte versionado;
- suíte de paridade interpretador × executável nativo para os casos compatíveis do Eixo B.

## Lacuna central

O caminho nativo atual ainda pressupõe Linux e o runtime Rust/C ABI do workspace. Um SO exige um caminho freestanding com entrada, layout, ligação, boot e validação próprios.

## Escada proposta

| Degrau | Entrega | Critério de pronto |
|---|---|---|
| BM1 | alvo freestanding x86-64 | compilar um programa sem libc, syscalls Linux ou runtime de processo hospedeiro |
| BM2 | emissão de objeto relocável | `pink` produz `.o` consumível por linker externo, com símbolos e relocations verificáveis |
| BM3 | seções e linker script | controle documentado de `.text`, `.rodata`, `.data`, `.bss`, endereço de carga e símbolo de entrada |
| BM4 | entrada e inicialização | `_start` ou equivalente configura stack mínima, zera `.bss` e chama uma função Pinker de entrada |
| BM5 | runtime freestanding mínimo | memória, saída serial e falha irrecuperável funcionam sem depender de `std`, libc ou Linux |
| BM6 | protocolo de boot | kernel Pinker inicializa por protocolo escolhido e documentado, sem acoplar a linguagem a um único bootloader |
| BM7 | imagem de kernel reproduzível | build produz imagem/ISO/disco determinístico com manifesto de artefatos |
| BM8 | execução em emulador | QEMU inicia a imagem, captura saída serial e retorna resultado verificável |
| BM9 | gate automatizado | CI executa teste bare-metal controlado e falha se boot, saída ou código esperado divergirem |

## Dependências obrigatórias

Antes do primeiro marco bare-metal útil, a Pinker precisa de:

- ponteiro de função materializado e chamada indireta;
- superfície `alocar`/`liberar` ou alocador freestanding equivalente;
- inline assembly com lowering real;
- operações de ponteiro e acesso volátil adequados ao recorte de hardware;
- controle de layout e ABI suficiente para tabelas e estruturas de boot;
- política explícita de abort/panic sem sistema hospedeiro.

## Relação com a Faixa 7

A trilha bare-metal cria o caminho de artefato e execução. A Faixa 7 fornece as capacidades físicas necessárias para tornar esse caminho útil:

- aritmética de ponteiros;
- structs empacotadas e layout controlado;
- bitfields;
- casts ponteiro ↔ inteiro;
- calling conventions;
- linker script e seções customizadas;
- operações atômicas;
- barriers e fences.

O item de linker script permanece na Faixa 7 como capacidade da linguagem/toolchain; esta trilha define como essa capacidade participa do bootstrap completo.

## Marcos corrigidos

- **Marco SO 0 — boot verificável:** imagem Pinker freestanding inicia em QEMU e produz saída serial sem Linux.
- **Marco SO 1 — núcleo de baixo nível:** programa bare-metal usa alocador próprio e handler de interrupção tipado.
- **Marco SO 2 — kernel mínimo:** scheduler, syscalls e primitivas básicas de sincronização executam sobre o ambiente próprio.
- **Marco SO 3 — dispositivos:** abstrações de dispositivo de bloco e caractere permitem armazenamento e console próprios.

## Limites honestos

- nenhuma etapa BM está implementada apenas por estar descrita aqui;
- o backend Linux atual continua sendo o único caminho nativo operacional até fase funcional específica;
- protocolo de boot, formato de imagem e emulador são decisões de implementação ainda abertas;
- esta trilha não substitui as Faixas 7, 10 ou 11;
- suporte multi-arquitetura permanece fora do primeiro recorte;
- cada degrau exige fase numerada, testes, documentação e lowering nativo correspondente.

## Regra de execução

A ordem BM1–BM9 é recomendada por dependência técnica. Alterações de ordem exigem justificativa no roadmap e não podem declarar um marco alcançado sem artefato executável e validação objetiva.