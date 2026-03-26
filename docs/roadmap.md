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

**Status**: bloco concluído. Fases 64–72 entregues. Bloco 7 é a trilha ativa seguinte.

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

---

### Bloco 7 — Backend nativo real

**Status**: **bloco suficientemente consolidado e encerrado como trilha ativa**. A consolidação das Fases 73–84 permitiu abrir o Bloco 8 sem declarar encerramento absoluto do Bloco 7.

**Objetivo geral**: transformar gradualmente o backend textual/experimental da Pinker em backend nativo real mais utilizável — capaz de gerar código que executa de verdade na máquina, com convenções de chamada e memória concretas.

**Itens do bloco (ordem interna sugerida)**:

1. **Subset real montável ampliado** — ampliar o subset do `--asm-s` para cobrir mais construtos do pipeline textual atual, gerando assembly que um assembler real aceite em mais casos.
2. **Convenção de chamada concreta mínima** — definir e registrar uma ABI mínima real (registradores, passagem de argumentos, valor de retorno) para o subset funcional alvo.
3. **Frame/registradores mínimos reais** — emitir prólogo/epílogo de frame real (salvar/restaurar registradores, ajustar stack pointer) no subset de funções contemplado.
4. **Chamadas reais no subset nativo** — lowering de `call` e `call_void` para instruções de chamada reais com ABI concreta no subset ativo.
5. **Memória real mínima no backend** — lowering de pelo menos um acesso de memória (load/store) para instruções de memória reais no subset do bloco.
6. **Artefato executável mais amplo** — avançar além do subset experimental da Fase 55; ampliar o que pode ser compilado, montado e executado de forma reproduzível.

#### Observação sobre numeração de fases

Os 6 itens acima representam a ordem interna sugerida do Bloco 7. A numeração exata de fase (Fase 73, Fase 74, …) será atribuída a cada item no momento de sua rodada funcional, conforme a convenção ativa.

---

### Bloco 8 — I/O e ecossistema útil

**Status**: **encerrado como trilha ativa por suficiência funcional/documental**. A consolidação das Fases 85–110 tornou o recorte de I/O/ecossistema útil suficientemente estável e auditável para encerrar o bloco sem exaustão total das possibilidades.

**Objetivo geral**: transformar a Pinker em linguagem mais interativa e útil para scripts, tooling e ecossistema — ampliando a superfície de I/O e tornando a linguagem mais utilizável no dia a dia.

**Itens do bloco (ordem interna sugerida)**:

1. **Entrada básica** — `ouvir` ou equivalente: leitura de entrada padrão em `--run` para pelo menos um tipo básico.
2. **Arquivo — leitura mínima** — `abrir`/`fechar` + leitura de conteúdo de arquivo simples.
3. **Arquivo — escrita mínima** — `escrever` para arquivo com semântica básica de abertura/fechamento.
4. **Verso operacional útil** — ampliar o subset operacional de `verso`: passagem por chamada, retorno, variável e operações mínimas além de `falar`.
5. **Operações mínimas de texto** — concatenação, comprimento e acesso por índice para `verso`.
6. **Melhorias em `falar`** — formatação mínima, múltiplos argumentos ou interpolação básica.
7. **Base para tooling em Pinker** — elementos mínimos que tornam a Pinker utilizável para escrever scripts e ferramentas simples.

#### Separação deliberada entre Bloco 7 e Bloco 8

- **Bloco 7** é a trilha de soberania/backend: a Pinker precisa gerar código real antes de expandir ecossistema.
- **Bloco 8** é a trilha de I/O/ecossistema útil: mais interatividade e utilidade para o programador.
- Manter essa separação evita misturar frentes com dependências e prioridades distintas.
- O Bloco 8 só deve ser aberto como trilha ativa após o Bloco 7 atingir consolidação suficiente.

#### Observação sobre numeração de fases

Status de execução no bloco:
- **Fase 85 concluída**: item 1 entregue no recorte mínimo, via intrínseca `ouvir()` em `--run` para `bombom` (u64), com diagnóstico claro para entrada inválida.
- **Fase 86 concluída**: item 2 entregue no recorte mínimo em `--run`, com `abrir("caminho") -> bombom`, `ler_arquivo(handle) -> bombom` e `fechar(handle)` para leitura simples de conteúdo inteiro em arquivo.
- **Fase 87 concluída**: item 3 entregue no recorte mínimo em `--run`, com `escrever(handle, bombom)` para sobrescrita simples do conteúdo do arquivo já aberto por `abrir(...)`, mantendo fechamento explícito com `fechar(handle)`.
- **Fase 88 concluída**: item 4 entregue no recorte mínimo em `--run`, com `verso` operacional em variável local, passagem por chamada, retorno e `falar(verso)` por valor.
- **Fase 89 concluída (parcial do item 5)**: concatenação mínima (`juntar_verso(verso, verso)`) e comprimento mínimo (`tamanho_verso(verso) -> bombom`) operacionais em `--run`.
- **Fase 90 concluída (fechamento do item 5)**: indexação mínima de `verso` em `--run` via `indice_verso(verso, bombom) -> verso`, com diagnóstico explícito de faixa e tipo no recorte mínimo.
- **Fase 91 concluída (item 6 no recorte mínimo)**: `falar` passou a aceitar múltiplos argumentos no `--run`, com mistura mínima heterogênea entre tipos já estáveis (incluindo `bombom` + `verso`) e separação previsível por espaço simples.
- **Fase 92 concluída (item 7 no recorte mínimo)**: base mínima de tooling em `--run` com `argumento(bombom) -> verso` para argv posicional e `sair(bombom)` para status explícito de saída, sem parser de flags/subcomandos/env.
- **Fase 93 concluída (refinamento mínimo pós-Fase 92)**: ergonomia mínima de argv em `--run` com `quantos_argumentos() -> bombom` e `tem_argumento(bombom) -> logica`, mantendo recorte de tooling pequeno e sem parser de flags/subcomandos/env.
- **Fase 94 concluída (refinamento mínimo pós-Fase 93)**: fallback posicional mínimo com `argumento_ou(bombom, verso) -> verso` em `--run`, reduzindo falha em scripts simples sem abrir parser amplo de CLI.
- **Fase 95 concluída (ambiente mínimo de processo em `--run`)**: leitura mínima de variável de ambiente com fallback via `ambiente_ou(verso, verso) -> verso` e leitura de diretório atual via `diretorio_atual() -> verso`, sem mutação/listagem de ambiente, sem `chdir` e sem biblioteca ampla de paths.
- **Fase 96 concluída (refinamento mínimo pós-Fase 95)**: introspecção mínima de caminho em `--run` com `caminho_existe(verso) -> logica` e classificação mínima `e_arquivo(verso) -> logica`, sem listagem de diretórios, sem `chdir`, sem globbing e sem biblioteca ampla de paths.
- **Fase 97 concluída (refinamento mínimo pós-Fase 96)**: classificação complementar com `e_diretorio(verso) -> logica` e composição mínima com `juntar_caminho(verso, verso) -> verso` em `--run`, sem canonicalização, sem normalização rica, sem listagem de diretórios e sem biblioteca ampla de paths.
- **Fase 98 concluída (refinamento mínimo pós-Fase 97)**: metadados mínimos de arquivo com `tamanho_arquivo(verso) -> bombom` e `e_vazio(verso) -> logica` em `--run`, sem timestamps, sem permissões, sem criação/remoção e sem biblioteca ampla de metadata/filesystem.
- **Fase 99 concluída (refinamento mínimo pós-Fase 98)**: mutação mínima e controlada de filesystem com `criar_diretorio(verso) -> nulo` e `remover_arquivo(verso) -> nulo` em `--run`, sem criação recursiva, sem remoção de diretório, sem rename/move/cópia e sem biblioteca ampla de filesystem.
- **Fase 100 concluída (refinamento mínimo pós-Fase 99)**: complemento mínimo de diretório + leitura textual mínima em `--run` com `remover_diretorio(verso) -> nulo` (somente diretório vazio, sem recursão) e `ler_verso_arquivo(handle) -> verso` (conteúdo textual completo do handle aberto por `abrir`), sem rename/move/cópia, sem listagem e sem streaming/append.
- **Fase 101 concluída (refinamento mínimo pós-Fase 100)**: escrita textual mínima em `--run` com `escrever_verso(handle, verso) -> nulo` e complemento mínimo operacional `criar_arquivo(verso) -> bombom` para criação + obtenção de handle no mesmo recorte; sem append, sem streaming, sem escrita por linha, sem encoding sofisticado e sem biblioteca ampla de filesystem/texto.
- **Fase 102 concluída (refinamento mínimo pós-Fase 101)**: truncamento mínimo em `--run` com `truncar_arquivo(handle) -> nulo`, validado explicitamente por `tamanho_arquivo(verso) -> bombom` e `e_vazio(verso) -> logica` no pós-estado (com releitura textual mínima do mesmo handle), sem truncamento por caminho, sem append, sem streaming, sem escrita por linha e sem biblioteca ampla de filesystem/texto.
- **Fase 103 concluída (refinamento mínimo pós-Fase 102)**: observação textual mínima em `--run` com `contem_verso(verso, verso) -> logica` e `comeca_com(verso, verso) -> logica`, integrada ao fluxo já existente de arquivo/argv/ambiente sem abrir API textual ampla (sem `termina_com`, split/replace/regex/trim).
- **Fase 104 concluída (refinamento mínimo pós-Fase 103)**: observação textual complementar mínima em `--run` com `termina_com(verso, verso) -> logica` e `igual_verso(verso, verso) -> logica`, fechando o conjunto mínimo de comparação/observação textual do bloco sem abrir API textual ampla (sem split/replace/regex/trim).
- **Fase 105 concluída (refinamento mínimo pós-Fase 104)**: saneamento textual mínimo em `--run` com `vazio_verso(verso) -> logica` (vazio exato) e `aparar_verso(verso) -> verso` (aparo de bordas), para permitir limpeza local de entrada sem abrir split/replace/regex/trim variants ou biblioteca textual ampla.
- **Fase 106 concluída (refinamento mínimo pós-Fase 105)**: normalização mínima de caixa em `--run` com `minusculo_verso(verso) -> verso` e `maiusculo_verso(verso) -> verso`, mantendo recorte local e auditável sem casefolding, sem locale-aware behavior e sem API textual ampla.
- **Fase 107 concluída (refinamento mínimo pós-Fase 106)**: observação textual posicional mínima em `--run` com `indice_verso_em(verso, verso) -> bombom` (primeira ocorrência; sentinela `u64::MAX` quando ausente) e ergonomia mínima de presença com `nao_vazio_verso(verso) -> logica`, sem abrir última/múltiplas ocorrências, regex, split/replace/slicing geral ou biblioteca textual ampla.
- **Fase 108 concluída (refinamento mínimo pós-Fase 107)**: append textual mínimo em `--run` com `abrir_anexo(verso) -> bombom` e `anexar_verso(bombom, verso) -> nulo`, com append por handle sem newline implícito e sem abrir append por caminho, múltiplos modos gerais, streaming, escrita por linha, seek/cursor público ou API ampla de filesystem/texto.
- **Fase 109 concluída (refinamento mínimo pós-Fase 108)**: leitura textual mínima direta por caminho em `--run` com `ler_arquivo_verso(verso) -> verso` e fallback ergonômico `arquivo_ou(verso, verso) -> verso`, com leitura completa por caminho e fallback textual para ausência/impossibilidade simples de leitura, sem streaming, sem escrita/append por caminho, sem modos ricos, sem seek/cursor e sem biblioteca ampla de filesystem/texto.
- **Fase 110 concluída (refinamento mínimo pós-Fase 109)**: entrada textual mínima em `--run` com `ouvir_verso() -> verso` e `ouvir_verso_ou(verso) -> verso`, com leitura textual única da stdin, remoção mínima de newline final e fallback textual simples para EOF/impossibilidade operacional simples, sem streaming, sem leitura não bloqueante, sem timeout, sem API rica de terminal e sem biblioteca ampla de entrada textual.

#### Encerramento formal do Bloco 8

- O Bloco 8 cumpriu sua função de ampliar a utilidade prática da Pinker em `--run` com I/O/ecossistema útil em recorte mínimo, auditável e historicamente consistente.
- O bloco consolidou `argv`/ambiente/path/arquivo/texto/entrada em subset funcional suficiente para scripts e tooling simples, sem competir com ecossistemas ricos.
- O encerramento é por **suficiência de trilha**, não por exaustão de possibilidades.
- Este encerramento **não proíbe** futuras fases relacionadas a I/O; apenas remove o tema como trilha principal ativa.
- Qualquer ampliação futura de I/O deve surgir subordinada à maturidade global do projeto, e não como continuação aberta imediata do Bloco 8.

---

### Bloco 9 — ampliação do backend nativo real

**Status**: **encerrado como trilha ativa por suficiência conservadora** (consolidação canônica após as Fases 111–119).

**Tese do bloco**:
- o backend nativo real já existe e é o ponto de partida;
- o Bloco 9 não reinicia backend;
- o objetivo é ampliar cobertura semântica real sustentada no backend nativo;
- foco em cobertura/robustez incremental do subset, não em performance, otimização ou backend pleno.

**Escada interna (ordem sugerida e auditável)**:

1. **9.1 — múltiplos blocos, labels e salto incondicional**
   - sair do modelo excessivamente linear/bloco único;
   - suporte mínimo a labels/blocos e transferência explícita de controle;
   - sem inflar condicionais nesta etapa.
2. **9.2 — branch condicional real**
   - comparações mínimas + `cmp`/`jcc` (ou equivalente no recorte adotado);
   - desvio condicional auditável, focado em controle de fluxo real.
3. **9.3 — loops reais**
   - `while`/looping mínimo coerente com a etapa de branch;
   - sem abrir subsistema complexo de controle de fluxo.
4. **9.4 — globais mínimas e base de `.rodata`**
   - armazenamento global mínimo;
   - base estrutural para constantes estáticas/`.rodata`;
   - preparação explícita para usos futuros (incluindo literais/strings mínimas) sem antecipar API ampla de strings.
5. **9.5 — ABI mínima mais larga, ainda conservadora**
   - ampliar capacidade de chamadas sem prometer ABI plena;
   - etapa mais larga que o subset anterior, mantendo recorte mínimo;
   - pode evoluir em duas camadas conservadoras em vez de salto único.
6. **9.6 — compostos mínimos no backend nativo real (fechado no recorte homogêneo conservador atual)**
   - fechamento conservador em `seta<bombom>` com `deref_load`/`deref_store` mínimos e offsets explícitos;
   - sem heterogeneidade, sem composto por valor na ABI, sem structs/arrays gerais e sem sistema geral de agregados.

**Status de execução no bloco**:
- **Fase 111 concluída (entrada inicial do item 9.1)**: backend nativo externo passou a aceitar múltiplos blocos por função com labels e `jmp` incondicional auditável, mantendo rejeição explícita para branch condicional (`talvez/senao`/`sempre que`) neste recorte.
- **Fase 112 concluída (entrada inicial do item 9.2)**: backend nativo externo passou a aceitar branch condicional mínimo auditável com comparação `==` e emissão `cmp`/`jcc`, preservando recorte pequeno e sem abrir loops.
- **Fase 113 concluída (entrada inicial do item 9.3)**: backend nativo externo passou a aceitar loops reais mínimos (`sempre que`) no recorte auditável com condição `==`/`<`, mantendo fora `break`/`continue` amplos e comparações gerais.
- **Fase 114 concluída (entrada inicial do item 9.4)**: backend nativo externo passou a aceitar globais estáticas mínimas (`eterno` literal `bombom`/`logica`) com emissão auditável de `.section .rodata` e leitura por símbolo no fluxo externo, sem abrir strings amplas nem sistema global rico.
- **Fase 115 concluída (primeira camada do item 9.5)**: backend nativo externo ampliou a ABI mínima de call direta de até 2 para até 3 argumentos `bombom` (`%rdi/%rsi/%rdx`), mantendo recusa explícita de 4+ argumentos e sem abrir ABI plena.
- **Fase 116 concluída (entrada inicial do item 9.6)**: backend nativo externo abriu composto mínimo camada 1 por ponteiro homogêneo (`seta<bombom>`) com `deref_load` auditável em função externa, mantendo fora compostos amplos e ABI composta por valor.
- **Fase 117 concluída (camada 2 conservadora do item 9.6)**: backend nativo externo ampliou o recorte homogêneo para aceitar local `seta<bombom>` no fluxo externo e leitura auditável de dois elementos via offset explícito mínimo (`base + 8` + `deref_load`), preservando fora composto por valor, structs/arrays gerais e ABI composta.
- **Fase 118 concluída (camada 3 conservadora do item 9.6)**: backend nativo externo ampliou o mesmo recorte homogêneo para aceitar `deref_store` mínimo (`*ptr = valor`) em `seta<bombom>`, preservando offset explícito auditável, sem abrir composto por valor, structs/arrays gerais, heterogeneidade ou ABI composta.
- **Fase 119 concluída (camada 4 conservadora do item 9.6)**: backend nativo externo consolidou o recorte homogêneo de `seta<bombom>` como par mínimo utilizável com sequência coesa de `deref_load` + `deref_store` + releitura auditável (offsets explícitos), sem abrir composto amplo, ABI composta ou layout geral.

#### Encerramento conservador do Bloco 9

- O Bloco 9 cumpriu sua função: ampliou de forma real, pequena e auditável o backend nativo externo já existente.
- O bloco **não** foi desenhado para virar backend pleno, nem para buscar otimização/performance ou runtime nativa grande.
- O encerramento é por **suficiência conservadora de trilha**, não por exaustão total do espaço de backend.
- O item **9.6** fica formalmente fechado **apenas** no recorte homogêneo conservador atual (`seta<bombom>` + `deref_load`/`deref_store` mínimos + offsets explícitos).
- Futuras ampliações de backend nativo externo continuam possíveis, porém subordinadas a outra maturidade do projeto, sem continuação automática e indefinida do Bloco 9.

#### Exclusões explícitas do Bloco 9

Por padrão, o Bloco 9 **não cobre**:
- backend nativo pleno;
- otimizador/otimizações relevantes;
- allocator completo;
- runtime grande;
- ABI ampla/plena;
- compostos por valor na ABI;
- retorno composto amplo;
- structs gerais;
- arrays gerais;
- compostos heterogêneos amplos;
- strings amplas;
- sistema geral de globais;
- layout/alinhamento geral sofisticado;
- subsistema amplo de strings;
- ecossistema de terminal rico;
- suporte geral a `sussurro` amplo;
- redesign completo da pipeline;
- performance tuning como objetivo principal;
- autohospedagem;
- independência total do backend para todos os recortes futuros da linguagem.


#### Próxima abertura funcional

- Após o encerramento conservador do Bloco 9, a próxima frente funcional fica **a definir conscientemente**.
- Não presumir continuidade automática de 9.6.
- Não reabrir o Bloco 9 por inércia documental; só por necessidade extraordinária, pequena e bem justificada.

#### Trava de runtime nativa mínima no Bloco 9

- qualquer runtime nativa mínima do Bloco 9 só pode entrar para desbloquear demonstração observável de capacidade semântica já conquistada no backend;
- a runtime serve ao backend;
- a runtime não pode sequestrar o bloco;
- a runtime não pode virar trilha paralela de conveniência.

---

### Bloco 10 — cobertura semântica do backend nativo

**Status**: **trilha ativa em execução (Doc-21; Fases 122–125 abriram 10.2 em camadas conservadoras 1, 2, 3 e 4; Fases 126–127 avançaram 10.3 em camadas conservadoras 1 e 2)**.

**Tese do bloco**:
- o backend nativo real já existe e é o ponto de partida;
- o Bloco 10 existe para ampliar quanto da semântica que a Pinker já domina em outras camadas pode ser sustentada honestamente no backend nativo;
- foco em cobertura semântica real, não em backend pleno;
- sem transformar o bloco em trilha de performance, otimizador, runtime grande ou “embelezamento” de compilador.

**Ordem interna canônica (refinada)**:
1. **10.1 — tipos inteiros mais largos**
   - ampliar o backend além do recorte escalar mínimo atual, com abertura pequena e auditável.
2. **10.2 — comparações ampliadas**
   - ampliar comparações além do recorte mínimo atual, com honestidade sobre signed/unsigned e sem abrir universo total de uma vez.
3. **10.3 — `quebrar` / `continuar`**
   - destravar controle de fluxo de laço em recorte mínimo e auditável, sem virar subsistema geral complexo.
4. **10.4 — `ninho` / compostos heterogêneos mínimos**
   - primeiro salto estrutural de heterogeneidade mínima; este item vem **antes** de `virar`.
5. **10.5 — `virar` / cast operacional mínimo**
   - cast útil no recorte escalar, posicionado após o primeiro salto estrutural de compostos heterogêneos mínimos.
6. **10.6 — `verso` mínima (condicional)**
   - item final e condicional do bloco, sem garantia de execução.

**Status de execução no bloco**:
- **Fase 120 concluída (entrada inicial do item 10.1)**: backend nativo externo abriu recorte mínimo e auditável para inteiro fixo adicional `u32` em parâmetros/locais no fluxo externo, preservando o subset anterior, sem abrir comparações ampliadas, casts amplos ou ABI plena.
- **Fase 121 concluída (camada 2 conservadora do item 10.1)**: backend nativo externo ampliou o mesmo recorte mínimo para aceitar também `u64` em parâmetros/locais, sem abrir comparações ampliadas (10.2), sem casts amplos e sem ABI plena.
- **Fase 122 concluída (entrada inicial do item 10.2)**: backend nativo externo abriu comparação ampliada mínima `!=` no fluxo externo, preservando recorte conservador (`==`, `!=`, `<`) sem abrir casts amplos, sem coerções implícitas e sem avançar 10.3.
- **Fase 123 concluída (camada 2 conservadora do item 10.2)**: backend nativo externo ampliou o recorte comparativo mínimo com `>` no mesmo fluxo externo, mantendo semântica relacional **não assinada** no subset vigente (`bombom`/`u32`/`u64`), sem abrir `<=`/`>=`, sem coerções implícitas, sem casts amplos e sem avançar 10.3.
- **Fase 124 concluída (camada 3 conservadora do item 10.2)**: backend nativo externo ampliou o recorte comparativo mínimo com `<=` no mesmo fluxo externo, mantendo semântica relacional **não assinada** no subset vigente (`bombom`/`u32`/`u64`), sem abrir `>=`, sem coerções implícitas, sem casts amplos e sem avançar 10.3.
- **Fase 125 concluída (camada 4 conservadora do item 10.2)**: backend nativo externo ampliou o recorte comparativo mínimo com `>=` no mesmo fluxo externo, mantendo semântica relacional **não assinada** no subset vigente (`bombom`/`u32`/`u64`), sem abrir pacote geral signed/unsigned, sem coerções implícitas, sem casts amplos e sem avançar 10.3.
- **Fase 126 concluída (camada 1 conservadora do item 10.3)**: backend nativo externo abriu recorte mínimo e auditável de `quebrar`/`continuar` no contexto de `sempre que` já suportado, reaproveitando labels/saltos já materializados no `selected`, sem abrir controle de fluxo geral, sem aninhamento amplo garantido e sem avançar 10.4.
- **Fase 127 concluída (camada 2 conservadora do item 10.3)**: backend nativo externo ampliou o mesmo recorte com aninhamento mínimo controlado de `sempre que` (laço interno em laço externo), com `quebrar`/`continuar` auditáveis no fluxo externo sem abrir controle de fluxo geral, sem abrir 10.4 e sem prometer suporte amplo de aninhamento.

#### Exclusões explícitas do Bloco 10

Por padrão, o Bloco 10 **não cobre**:
- backend nativo pleno;
- otimizador relevante;
- performance tuning como objetivo principal;
- runtime grande;
- ABI ampla/plena;
- strings amplas por padrão;
- sistema geral de texto;
- sistema geral de compostos avançados;
- redesign completo da pipeline;
- autohospedagem;
- independência total do backend para todos os recortes da linguagem;
- abertura simultânea de muitos tipos/semânticas para “fechar bloco rápido”.

#### Trava específica para 10.6 (`verso`)

- `verso` mínima no backend nativo só pode entrar no **fim** do bloco e apenas se ainda houver chão técnico e documental.
- Mesmo recorte pequeno de `verso` arrasta representação, `.rodata`, calling convention textual e expectativa de operações futuras.
- Por isso, `verso` é item **condicional** e não pode sequestrar o bloco.
- O Bloco 10 pode ser considerado bem-sucedido mesmo sem chegar a `verso`.

---

## Interpretação obrigatória da trilha

- Bloco 10 é a trilha ativa atual com foco disciplinado em cobertura semântica do backend nativo (sem backend pleno).

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
