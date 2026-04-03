# Inventário canônico de intrínsecas — Fase 180

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

> **Contexto:** este documento é o entregável fundacional da Fase 180 (abertura do Bloco 18 — core nobre e bibliotecas temáticas). Ele cataloga todas as intrínsecas operacionais da Pinker v0 com classificação inicial por natureza funcional e status proposto, servindo de base factual para as decisões das próximas fases do bloco.

## 1. Escopo e convenções

### 1.1 O que este inventário cobre

- Funções intrínsecas do runtime (`interpreter.rs` via `try_call_intrinsic`).
- Funções intrínsecas reconhecidas na semântica (`semantic.rs` via `check_call_expr`).
- Aliases legados provisórios com referência ao nome canônico.
- Intrínsecas internas usadas pelo lowering de `para cada`.

### 1.2 O que este inventário NÃO cobre

- **`falar`**: é statement da linguagem (keyword, `Stmt::Falar`), não função intrínseca. Tem nó AST próprio, lowering próprio e instruções de máquina dedicadas (`PrintIntInline`, etc.). Pertence ao **núcleo da linguagem** por definição.
- **`peso`** e **`alinhamento`**: são expressões de introspecção de tipo (`ExprKind::SizeOfType`, `ExprKind::AlignOfType`), resolvidas em tempo de compilação na semântica. Não são intrínsecas de runtime. Pertencem ao **núcleo da linguagem** por definição.
- **Construtos sintáticos**: `para cada`, `sempre que`, `talvez`/`senao`, `quebrar`, `continuar`, `eterno`, `nova`, `muda`, `mimo`, `trazer`, `pacote`, `carinho`, `principal`, `apelido`, `ninho`, `seta`, `virar`, `fragil`, `sussurro`, `livre` — são todos construtos do núcleo da linguagem, não intrínsecas.

### 1.3 Convenção de assinatura

- `→ nulo` indica que a função não retorna valor utilizável.
- `[arg]` indica argumento opcional (aridade variável).
- Tipos: `bombom` (u64), `verso` (string), `logica` (bool), `lista<bombom>`, `mapa<verso,bombom>`.

## 2. Critérios de classificação

### 2.1 Núcleo nobre

Uma intrínseca pertence ao núcleo nobre quando satisfaz, em conjunto:

1. **Indispensabilidade** — sem ela, programas triviais não funcionam.
2. **Universalidade** — usada na grande maioria dos programas Pinker.
3. **Acoplamento ao motor** — depende diretamente de mecânica do compilador/runtime, não apenas de chamada de sistema.
4. **Irredutibilidade** — não pode ser implementada como função Pinker sobre outras intrínsecas.

### 2.2 Família temática candidata

Uma intrínseca tende a família temática quando:

1. **Domínio funcional reconhecível** — pertence a um agrupamento coeso de funcionalidades.
2. **Ganho de clareza com contexto** — vive melhor sob prefixo de domínio do que como global solta.
3. **Não precisa estar "no ar"** — programas que não usam o domínio não precisam dela.
4. **Inflação lexical atual** — seu nome atual é longo ou verboso por falta de namespace próprio.

### 2.3 Domínio provisório

Uma intrínseca recebe domínio provisório quando:

1. Pertence a um agrupamento funcional reconhecível, mas...
2. O nome da família ainda não foi canonizado pela camada Rosa, ou...
3. Há dúvida legítima sobre o melhor agrupamento.

### 2.4 Construtos do núcleo que operam sobre tipos de família

Construtos sintáticos como `para cada` permanecem núcleo da linguagem mesmo quando operam sobre tipos ligados a futuras famílias (ex.: `lista<bombom>`, `mapa<verso,bombom>`). A separação do Bloco 18 é sobre intrínsecas, não sobre rebaixar construtos de linguagem.

## 3. Inventário completo

### 3.1 Núcleo nobre — intrínsecas

| # | Nome | Assinatura | Origem | Nota |
|---|------|-----------|--------|------|
| 1 | `ouvir` | `() → bombom` | Bloco 8, Fase 85 | Entrada fundamental de `bombom` pela stdin |
| 2 | `ouvir_verso` | `() → verso` | Bloco 8, Fase 110 | Entrada fundamental textual pela stdin |
| 3 | `ouvir_verso_ou` | `(verso) → verso` | Bloco 8, Fase 110 | Entrada textual com fallback para EOF |
| 4 | `sair` | `(bombom) → nulo` | Bloco 8, Fase 92 | Controle fundamental de processo |

**Justificativa resumida:** estas intrínsecas são universais, indispensáveis para I/O básico e controle de processo, acopladas ao runtime e irredutíveis. Junto com `falar` (statement), `peso` e `alinhamento` (expressões compiletime), formam o núcleo mínimo da linguagem.

### 3.2 Família candidata: texto

Domínio: manipulação de `verso` (strings).

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 5 | `juntar_verso` | `(verso, verso) → verso` | Bloco 8, Fase 89 |
| 6 | `tamanho_verso` | `(verso) → bombom` | Bloco 8, Fase 89 |
| 7 | `indice_verso` | `(verso, bombom) → verso` | Bloco 8, Fase 90 |
| 8 | `contem_verso` | `(verso, verso) → logica` | Bloco 8, Fase 103 |
| 9 | `comeca_com` | `(verso, verso) → logica` | Bloco 8, Fase 103 |
| 10 | `termina_com` | `(verso, verso) → logica` | Bloco 8, Fase 104 |
| 11 | `igual_verso` | `(verso, verso) → logica` | Bloco 8, Fase 104 |
| 12 | `vazio_verso` | `(verso) → logica` | Bloco 8, Fase 105 |
| 13 | `aparar_verso` | `(verso) → verso` | Bloco 8, Fase 105 |
| 14 | `minusculo_verso` | `(verso) → verso` | Bloco 8, Fase 106 |
| 15 | `maiusculo_verso` | `(verso) → verso` | Bloco 8, Fase 106 |
| 16 | `indice_verso_em` | `(verso, verso) → bombom` | Bloco 8, Fase 107 |
| 17 | `buscar_verso` | `(verso, verso) → bombom` | Bloco 11, Fase 140 |
| 18 | `nao_vazio_verso` | `(verso) → logica` | Bloco 8, Fase 107 |
| 19 | `dividir_verso_em` | `(verso, verso, bombom) → verso` | Bloco 11, Fase 137 |
| 20 | `dividir_verso_contar` | `(verso, verso) → bombom` | Bloco 11, Fase 137 |
| 21 | `substituir_verso` | `(verso, verso, verso) → verso` | Bloco 11, Fase 138 |
| 22 | `juntar_verso_com` | `(verso, verso, verso) → verso` | Bloco 11, Fase 139 |

**Total: 18 intrínsecas.** Domínio coeso e com inflação lexical visível (prefixos/sufixos `_verso` repetidos). Candidata forte a família.

### 3.3 Família candidata: arquivo

Domínio: operações de arquivo por handle e por caminho.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 23 | `abrir` | `(verso) → bombom` | Bloco 8, Fase 86 |
| 24 | `fechar` | `(bombom) → nulo` | Bloco 8, Fase 86 |
| 25 | `ler_arquivo` | `(bombom) → bombom` | Bloco 8, Fase 86 |
| 26 | `ler_verso_arquivo` | `(bombom) → verso` | Bloco 8, Fase 100 |
| 27 | `ler_arquivo_verso` | `(verso) → verso` | Bloco 8, Fase 109 |
| 28 | `arquivo_ou` | `(verso, verso) → verso` | Bloco 8, Fase 109 |
| 29 | `escrever` | `(bombom, bombom) → nulo` | Bloco 8, Fase 87 |
| 30 | `escrever_verso` | `(bombom, verso) → nulo` | Bloco 8, Fase 101 |
| 31 | `criar_arquivo` | `(verso) → bombom` | Bloco 8, Fase 101 |
| 32 | `truncar_arquivo` | `(bombom) → nulo` | Bloco 8, Fase 102 |
| 33 | `abrir_anexo` | `(verso) → bombom` | Bloco 8, Fase 108 |
| 34 | `anexar_verso` | `(bombom, verso) → nulo` | Bloco 8, Fase 108 |

**Total: 12 intrínsecas.** Domínio coeso em torno de I/O de arquivo. Candidata forte.

### 3.4 Família candidata: caminho

Domínio: introspecção e manipulação de caminhos no filesystem.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 35 | `caminho_existe` | `(verso) → logica` | Bloco 8, Fase 96 |
| 36 | `e_arquivo` | `(verso) → logica` | Bloco 8, Fase 96 |
| 37 | `e_diretorio` | `(verso) → logica` | Bloco 8, Fase 97 |
| 38 | `juntar_caminho` | `(verso, verso) → verso` | Bloco 8, Fase 97 |
| 39 | `tamanho_arquivo` | `(verso) → bombom` | Bloco 8, Fase 98 |
| 40 | `e_vazio` | `(verso) → logica` | Bloco 8, Fase 98 |
| 41 | `criar_diretorio` | `(verso) → nulo` | Bloco 8, Fase 99 |
| 42 | `remover_arquivo` | `(verso) → nulo` | Bloco 8, Fase 99 |
| 43 | `remover_diretorio` | `(verso) → nulo` | Bloco 8, Fase 100 |
| 44 | `diretorio_atual` | `() → verso` | Bloco 8, Fase 95 |

**Total: 10 intrínsecas.** Domínio coeso. Candidata forte.

### 3.5 Família candidata: processo

Domínio: execução e integração com processos externos.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 45 | `executar_processo` | `(verso [, verso]) → bombom` | Bloco 15, Fase 161; argv1 na Fase 168 |
| 46 | `executar_com_entrada` | `(verso, verso [, verso]) → bombom` | Bloco 15, Fase 165; argv1 na Fase 177 |
| 47 | `pipeline_minimo` | `(verso, verso) → bombom` | Bloco 15, Fase 166 |
| 48 | `capturar_stdout` | `(verso [, verso]) → verso` | Bloco 15, Fase 163; argv1 na Fase 169 |
| 49 | `capturar_stderr` | `(verso [, verso]) → verso` | Bloco 15, Fase 164; argv1 na Fase 170 |

**Total: 5 intrínsecas.** Domínio muito coeso. Candidata forte.

### 3.6 Família candidata: tempo

Domínio: operações temporais.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 50 | `tempo_unix` | `() → bombom` | Bloco 14, Fase 160 |
| 51 | `formatar_tempo_unix` | `(bombom) → verso` | Bloco 14, Fase 160 |

**Total: 2 intrínsecas.** Domínio coeso, família pequena, dívida lexical visível (redundância `tempo_` no nome). Candidata à família-piloto do bloco.

### 3.7 Família candidata: ambiente

Domínio: argumentos de linha de comando, variáveis de ambiente e contexto de execução.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 52 | `argumento` | `(bombom) → verso` | Bloco 8, Fase 92 |
| 53 | `argumento_ou` | `(bombom, verso) → verso` | Bloco 8, Fase 94 |
| 54 | `quantos_argumentos` | `() → bombom` | Bloco 8, Fase 93 |
| 55 | `tem_argumento` | `(bombom) → logica` | Bloco 8, Fase 93 |
| 56 | `tem_chave` | `(verso) → logica` | Bloco 11, Fase 141 (FE-1) |
| 57 | `pedir_argumento` | `(verso, verso) → verso` | Bloco 11, Fase 141 (FE-1) |
| 58 | `tem_flag` | `(verso) → logica` | Bloco 11, Fase 143 |
| 59 | `ambiente_ou` | `(verso, verso) → verso` | Bloco 8, Fase 95 |
| 60 | `buscar_contexto` | `(verso, verso, verso) → verso` | Bloco 11, Fase 141 (FE-1) |

**Total: 9 intrínsecas.** Domínio coeso. Candidata forte.

### 3.8 Família candidata: acaso

Domínio: pseudoaleatoriedade.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 61 | `aleatorio_criar` | `(bombom) → bombom` | Bloco 13, Fase 156 |
| 62 | `aleatorio_proximo` | `(bombom) → bombom` | Bloco 13, Fase 156 |

**Total: 2 intrínsecas.** Domínio coeso, família pequena. Candidata.

### 3.9 Domínio provisório: colecao

Nome de família **em avaliação lexical** — não canonizado nesta fase.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 63 | `lista_bombom_criar` | `() → lista<bombom>` | Bloco 13, Fase 149 |
| 64 | `lista_bombom_anexar` | `(lista<bombom>, bombom) → nulo` | Bloco 13, Fase 149 |
| 65 | `lista_bombom_obter` | `(lista<bombom>, bombom) → bombom` | Bloco 13, Fase 149 |
| 66 | `lista_bombom_tamanho` | `(lista<bombom>) → bombom` | Bloco 13, Fase 149 |
| 67 | `lista_bombom_definir` | `(lista<bombom>, bombom, bombom) → nulo` | Bloco 13, Fase 150 |
| 68 | `lista_bombom_tirar_ultimo` | `(lista<bombom>) → bombom` | Bloco 13, Fase 151 |
| 69 | `mapa_verso_bombom_criar` | `() → mapa<verso,bombom>` | Bloco 13, Fase 152 |
| 70 | `mapa_verso_bombom_definir` | `(mapa<verso,bombom>, verso, bombom) → nulo` | Bloco 13, Fase 152 |
| 71 | `mapa_verso_bombom_obter` | `(mapa<verso,bombom>, verso) → bombom` | Bloco 13, Fase 152 |
| 72 | `mapa_verso_bombom_tem` | `(mapa<verso,bombom>, verso) → logica` | Bloco 13, Fase 152 |
| 73 | `mapa_verso_bombom_tamanho` | `(mapa<verso,bombom>) → bombom` | Bloco 13, Fase 152 |

**Total: 11 intrínsecas.** Domínio funcional claro, mas nome `colecao` em avaliação lexical. Domínio provisório atribuído internamente; canonização pública adiada.

### 3.10 Domínio provisório: formato

Nome de família **em avaliação lexical** — não canonizado nesta fase.

| # | Nome | Assinatura | Origem |
|---|------|-----------|--------|
| 74 | `formatar_verso` | `(verso, bombom\|verso [, bombom\|verso]) → verso` | Bloco 14, Fase 157 |
| 75 | `ler_linha_csv_bombom` | `(verso, verso) → lista<bombom>` | Bloco 14, Fase 158 |
| 76 | `emitir_linha_csv_bombom` | `(lista<bombom>, verso) → verso` | Bloco 14, Fase 158 |
| 77 | `ler_json_plano_bombom` | `(verso) → mapa<verso,bombom>` | Bloco 14, Fase 159 |
| 78 | `emitir_json_plano_bombom` | `(mapa<verso,bombom>) → verso` | Bloco 14, Fase 159 |

**Total: 5 intrínsecas.** Domínio funcional reconhecível, mas nome `formato` em avaliação lexical (vago; pode ser desdobrado em famílias menores no futuro). Domínio provisório atribuído; canonização pública adiada.

**Nota sobre `formatar_verso`:** esta intrínseca tem fronteira ambígua entre `texto` e `formato`. Pode pertencer a qualquer uma; a decisão final fica para a fase de canonização de famílias (18.2).

### 3.11 Aliases legados provisórios

| Nome legado | Nome canônico | Assinatura | Origem |
|---|---|---|---|
| `tem_argumento_nomeado` | `tem_chave` | `(verso) → logica` | FE-1 |
| `argumento_nomeado_ou` | `pedir_argumento` | `(verso, verso) → verso` | FE-1 |
| `argumento_nomeado_ou_ambiente_ou` | `buscar_contexto` | `(verso, verso, verso) → verso` | FE-1 |

Estes aliases existem por compatibilidade temporária e devem ser tratados como legado, não como superfície canônica.

### 3.12 Intrínsecas internas (não públicas)

| Nome | Assinatura | Propósito |
|---|---|---|
| `__pinker_internal_mapa_verso_bombom_iterador_criar` | `(mapa<verso,bombom>) → bombom` | Suporte interno ao lowering de `para cada chave em mapa` |
| `__pinker_internal_mapa_verso_bombom_iterador_proxima_chave` | `(bombom) → verso` | Suporte interno ao lowering de `para cada chave em mapa` |

Estas intrínsecas não fazem parte da superfície pública e não participam da classificação do Bloco 18. São mecanismo interno do compilador para desdobrar `para cada` sobre `mapa<verso,bombom>`.

## 4. Resumo quantitativo

| Classificação | Quantidade |
|---|---|
| Núcleo nobre — construtos de linguagem (não intrínsecas) | 3 (`falar`, `peso`, `alinhamento`) |
| Núcleo nobre — intrínsecas | 4 |
| Família candidata: texto | 18 |
| Família candidata: arquivo | 12 |
| Família candidata: caminho | 10 |
| Família candidata: processo | 5 |
| Família candidata: tempo | 2 |
| Família candidata: ambiente | 9 |
| Família candidata: acaso | 2 |
| Domínio provisório: colecao | 11 |
| Domínio provisório: formato | 5 |
| Aliases legados | 3 |
| Intrínsecas internas | 2 |
| **Total de intrínsecas públicas distintas** | **78** |

## 5. Observações para as próximas fases

1. **A classificação inicial é proposta, não definitiva.** A canonização de famílias (18.2) pode mover intrínsecas entre domínios.
2. **`formatar_verso` tem fronteira ambígua** entre `texto` e `formato`. Decisão adiada.
3. **`colecao` e `formato` são domínios provisórios** cujos nomes aguardam revisão lexical mais fria antes de canonização pública.
4. **O núcleo nobre é deliberadamente pequeno** (4 intrínsecas + 3 construtos). Isso é intencional: um core digno é um core enxuto.
5. **Nenhuma família foi operacionalizada nesta fase.** O inventário é base factual para decisões, não implementação de resolução qualificada.
6. **Aliases legados não devem ser propagados** para novas superfícies de família. Quando famílias forem operacionalizadas, apenas o nome canônico deve participar.
