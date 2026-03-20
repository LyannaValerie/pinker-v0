# Roadmap macro da Pinker (trilha oficial ativa)

Este documento passa a ser o **documento mestre** para voltar aos trilhos da Pinker v0.

- Não haverá mais projeto/trilha paralela.
- A Pinker seguirá uma trilha única de execução.
- As próximas fases funcionais devem respeitar a ordem real de dependências.
- `docs/future.md` continua existindo como inventário amplo, mas **não dita a ordem ativa**.

## Estado atual real (resumo)

A base atual está estável em pipeline textual (`semântica -> IR -> CFG -> selected -> Machine -> pseudo-asm`) com execução via interpretador (`--run`) e cobertura de testes ampla. Bloco 5 encerrado na Fase 63 (`pink build` / tooling de projeto). Os blocos 1–5 consolidaram: tipos, memória explícita (semântica/tipo), backend textual, bare metal textual e tooling mínimo. A fundação ausente crítica é o **modelo de memória operacional em runtime/pipeline**: vários construtos existem no tipo/semântica mas não executam ainda.

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

### Bloco 6 — Memória operacional

**Status**: bloco oficial ativo. Bloco 5 foi encerrado na Fase 63. Esta é a próxima trilha oficial da Pinker.

**Tese estratégica**: a Pinker já possui vários construtos parcialmente implementados (`seta<T>`, `ninho`, arrays fixos, `virar`, `fragil`, `sussurro`, kernel/freestanding textual), mas a maior parte deles ainda depende de uma fundação ausente: o modelo de memória operacional em runtime/pipeline. O Bloco 6 prioriza fechar essa base comum antes de abrir novas frentes horizontais.

**Por que Bloco 6 vem agora**: após Bloco 5 (tooling), a Pinker tem módulos, strings, I/O básico e build mínimo. O próximo passo estrutural não é expandir horizontalmente (terminal próprio, ecossistema soberano), mas sim operacionalizar a memória: sem dereferência real, ponteiros são decoração; sem escrita indireta, structs são categorias de tipo; sem aritmética de ponteiros, arrays são só tipos estáticos.

#### A. Itens que valem ser fechados cedo (parciais autocontidos)

Esses itens são quase autocontidos e desbloqueiam outros com custo menor:

1. **signed real no runtime** — tipos `i8`–`i64` estão bloqueados no `--run` (HF-3) por falta de representação correta; fechar isso remove um bloqueio crônico em falar/runtime.
2. **representação mínima de ponteiro no runtime** — `seta<T>` existe como tipo; uma representação mínima operacional (ex.: endereço abstrato ou índice de slot) é pré-requisito para os demais itens do bloco.

#### B. Núcleo do bloco (itens estruturais)

Esses itens formam a espinha dorsal do Bloco 6, em ordem interna sugerida de dependência:

3. **dereferência de leitura** — ler valor pelo endereço apontado por `seta<T>` em runtime/pipeline.
4. **escrita indireta via ponteiro** — escrever valor em endereço apontado por `seta<T>` (sem escrita, ponteiros são read-only; sem escrita, `ninho` via ponteiro é inviável).
5. **aritmética de ponteiros** — `seta + offset`, percorrer memória e arrays por ponteiro.
6. **acesso a campo operacional em `ninho`** — lowering de leitura e escrita de campo com layout real (offset por campo); `ninho` já tem layout estático (`peso`/`alinhamento`), falta o acesso operacional.
7. **indexação operacional em arrays** — lowering de leitura e escrita por índice com aritmética de ponteiro/offset; arrays fixos já têm tipo, falta execução real.
8. **cast operacional útil ligado à memória** — lowering de `virar` em CFG/Machine/runtime para o subset inteiro→inteiro já aprovado semanticamente; cast de/para ponteiro como extensão natural.
9. **primeiro efeito operacional real de `fragil`** — `fragil seta<T>` como qualificador semântico já propagado no pipeline; o efeito operacional mínimo (barrier/fence textual ou anotação de acesso não-otimizável) fecha o ciclo do construto.

#### Escopo deliberadamente fora do Bloco 6

Os itens abaixo **não são prioridade imediata** deste bloco e devem permanecer em `docs/future.md` sem competir com a trilha ativa:

- terminal próprio
- persona/diagnóstico mais vivo
- formatos tipo JSON/XML próprios
- biblioteca rica de strings (`ouvir`, `abrir`, `fechar`, `escrever`, formatação avançada)
- self-hosting
- backend nativo completo (x86_64 real, ELF, register allocation)
- kernel real robusto (GRUB/QEMU/ISO, Multiboot completo, runtime bare-metal amplo)
- package manager / ecossistema soberano completo

#### Observação sobre numeração de fases

Os 9 itens acima representam a **ordem interna sugerida** do Bloco 6. A numeração exata de fase (Fase 64, Fase 65, …) será atribuída a cada item no momento de sua rodada funcional, conforme a convenção ativa (Fase N = entrega funcional real). Esta rodada documental não atribui números de fase antecipados.

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
