# Roadmap macro da Pinker (trilha oficial ativa)

Este documento passa a ser o **documento mestre** para voltar aos trilhos da Pinker v0.

- Não haverá mais projeto/trilha paralela.
- A Pinker seguirá uma trilha única de execução.
- As próximas fases funcionais devem respeitar a ordem real de dependências.
- `docs/future.md` continua existindo como inventário amplo, mas **não dita a ordem ativa**.

## Estado atual real (resumo)

A base atual está estável em pipeline textual (`semântica -> IR -> CFG -> selected -> Machine -> pseudo-asm`) com execução via interpretador (`--run`) e cobertura de testes ampla. Ainda não há backend nativo real nem I/O de linguagem; ponteiros, structs, módulos/imports e strings mínimas já existem com escopos controlados.

## Trilha oficial consolidada (ordem do mais essencial ao mais complexo)

### Bloco 1 — fundação imediata da linguagem de sistemas
1. operador `%` nativo
2. inteiros unsigned com largura fixa (`u8`, `u16`, `u32`, `u64`)
3. inteiros signed com largura fixa (`i8`, `i16`, `i32`, `i64`)
4. aliases de tipo
5. arrays fixos
6. structs

### Bloco 2 — memória explícita
1. ponteiros
2. acesso a campo e indexação
3. casts controlados
4. `sizeof` / alinhamento
5. `volatile`

### Bloco 3 — saída do interpretador
1. backend textual `.s`
2. ABI mínima
3. uso de assembler/linker externo

### Bloco 4 — bare metal / kernel
1. inline asm
2. freestanding / no-std
3. linker script / boot entry
4. primeiro kernel mínimo

### Bloco 5 — tooling / ecossistema
1. módulos/imports
2. strings
3. I/O básico
4. `pink build` / tooling de projeto

## Interpretação obrigatória da trilha

- `%` nativo é a menor fase útil imediata.
- inteiros com largura fixa são o primeiro grande passo estrutural.
- arrays fixos e structs vêm antes de memória explícita mais pesada.
- backend nativo não deve vir antes da base mínima de tipos/modelagem.
- assembly textual `.s` é a estratégia inicial preferível antes de ELF direto.
- módulos/imports, strings e I/O são importantes, mas não devem atropelar a trilha de kernel neste momento.
- tooling próprio vem depois da base da linguagem estar sólida.

### Exceção controlada

`módulos/imports` podem ser antecipados **apenas** se a complexidade de desenvolvimento/teste da própria Pinker tornar o projeto monolítico inviável; mesmo nessa exceção, sem desviar a prioridade principal da trilha de kernel.

## Critério de bloco concluído

Um bloco só pode ser considerado suficientemente concluído para liberar o próximo quando:

- os itens previstos para esse bloco estiverem implementados no escopo combinado da trilha ativa;
- houver cobertura de testes proporcional nas camadas afetadas;
- `cargo build` e `cargo test` passarem sem regressões;
- não houver bloqueio semântico/estrutural conhecido dentro do próprio bloco que inviabilize o seguinte;
- o handoff e o estado operacional reflitam explicitamente que o bloco foi fechado ou parcialmente fechado.

## Regra de transição

- não iniciar fase do bloco seguinte enquanto houver item bloqueante pendente no bloco atual;
- itens paralelos/não bloqueantes podem ser adiados, desde que sejam registrados como tal.

## Relação operacional com docs/future.md

- `docs/future.md` = inventário amplo de possibilidades.
- `docs/roadmap.md` = ordem oficial ativa de implementação.
