# Pinker v0

Pinker v0 é a base factual atual da linguagem Pinker: um frontend pequeno e congelado em Rust, com foco em um recorte implementado e auditável.

O projeto separa explicitamente estado real, trilha ativa e direção identitária. Para o enquadramento canônico do repositório, consulte `docs/atlas.md`, `docs/agent_state.md`, `docs/roadmap.md` e `docs/history.md`.

## O que o frontend faz hoje

A lista abaixo resume apenas o que já está implementado no workspace atual, sem misturar visão futura com recurso pronto.

- léxico com spans
- parser para `pacote`, `trazer`, `carinho`, `mimo`, `talvez/senão`, `sempre que`, `eterno`, `nova`, `muda`
- tipos `bombom`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` e `logica`
- aliases de tipo (`apelido`), arrays fixos (`[tipo; N]`), structs (`ninho`), ponteiros (`seta<tipo>`)
- representação mínima de ponteiro no runtime (`RuntimeValue::Ptr`) para `seta<T>` no `--run`
- dereferência de leitura mínima com `*p` para `seta<bombom>` no `--run`
- escrita indireta mínima com `*p = valor` para `seta<bombom>` no `--run`
- aritmética mínima de ponteiro no runtime com `seta<bombom> + bombom` e `seta<bombom> - bombom` no `--run`
- acesso operacional mínimo a campo de `ninho` no runtime via `(*ptr).campo`, respeitando offsets de layout estático no subset atual
- indexação operacional mínima de arrays no runtime para o subset `[bombom; N]` com índice `bombom`, cobrindo `(*ptr)[i]` e também array por valor `a[i]` no recorte conservador da Fase 147
- qualificador `fragil` (`volatile`) para ponteiros explícitos (`fragil seta<tipo>`), com efeito operacional mínimo em `deref_load`/`deref_store` via caminho distinto no pipeline/runtime para o subset `fragil seta<bombom>`
- inline asm mínimo como statement textual com `sussurro("...")` (ou múltiplas strings), preservado até IR
- marca de unidade freestanding/no-std com `livre;` no topo do programa
- cast explícito controlado com `virar` (operacional em `--run` para inteiro->inteiro e `bombom <-> seta<bombom>`)
- consultas estáticas de layout com `peso(tipo)` e `alinhamento(tipo)`
- módulos/imports mínimos com `trazer modulo;` e `trazer modulo.simbolo;` (carregando `modulo.pink` no mesmo diretório do arquivo principal, com subset de import para `carinho` e `eterno`)
- strings mínimas como valor de linguagem com tipo `verso` e literal `"texto"` (frontend + semântica + IR)
- `verso` operacional mínimo em `--run`: variável local, passagem por chamada, retorno e uso em `falar(verso)`
- operações mínimas de texto em `--run` com `verso`: `juntar_verso(a, b)` para concatenação (apenas `verso + verso` via intrínseca), `tamanho_verso(v)` retornando `bombom` e indexação mínima por intrínseca `indice_verso(v, i)` (`verso`, `bombom`) retornando `verso` unitário
- saída básica com `falar(arg1, arg2, ...);` no `--run`, com múltiplos argumentos e separação por espaço simples no subset tipado (`bombom`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `logica`, `verso`)
- entrada básica com intrínseca `ouvir()` em `--run`, com leitura de stdin para `bombom` (u64) no recorte mínimo da Fase 85
- entrada textual mínima em `--run` com `ouvir_verso() -> verso` e `ouvir_verso_ou(verso) -> verso`, com remoção mínima de newline final e fallback simples em EOF/impossibilidade operacional simples (Fase 110)
- leitura mínima de arquivo em `--run` com intrínsecas `abrir("caminho") -> bombom`, `ler_arquivo(handle) -> bombom` e `fechar(handle)` (Fase 86)
- escrita mínima de arquivo em `--run` com intrínseca `escrever(handle, bombom)` após `abrir("caminho")`, com fechamento explícito via `fechar(handle)` (Fase 87)
- base mínima de tooling em `--run` com `argumento(i)` para argv posicional e `sair(codigo)` para status explícito de saída (Fase 92)
- ergonomia mínima de argv em `--run` com `quantos_argumentos()` e `tem_argumento(i)` para contagem/presença posicional sem coleção ampla (Fase 93)
- refinamento mínimo de fallback de argv em `--run` com `argumento_ou(i, padrao)` para script simples sem falha por ausência posicional (Fase 94)
- ergonomia prática mínima de argv nomeado em `--run` com `tem_chave(chave)` e `pedir_argumento(chave, padrao)`, mantendo compatibilidade temporária com `tem_argumento_nomeado`/`argumento_nomeado_ou`, e suportando apenas `--chave valor` e `--chave=valor` sem parser amplo (Fase 141 + FE-1)
- flags booleanas mínimas em `--run` com `tem_flag(chave)`, detectando presença literal de flags como `--quiet`/`--verbose` sem consumir valor seguinte e sem inferir presença a partir de `--chave=valor` (Fase 142)
- ambiente mínimo de processo em `--run` com `ambiente_ou(chave, padrao)` para leitura de variável de ambiente com fallback de `verso` (Fase 95)
- prioridade mínima entre argumento nomeado, ambiente e fallback textual em `--run` com `buscar_contexto(chave_arg, chave_env, padrao)`, mantendo compatibilidade temporária com `argumento_nomeado_ou_ambiente_ou`, e preservando a ordem `CLI -> ambiente -> padrão` sem parser/configuração ampla (Fase 143 + FE-1)
- execução mínima de processo externo em `--run` com `executar_processo(comando) -> bombom`, sem shell implícito, sem argumentos ricos e retornando apenas o código de saída do processo; a Fase 162 corrige testes e exemplos para usar binários auxiliares do próprio repositório em vez de depender de `/bin/true` e `/bin/false`
- argv explícito mínimo em processo externo no `--run` com `executar_processo(comando, argv1) -> bombom`, aceitando exatamente um argumento textual adicional sem shell implícito, sem quoting amplo e sem listas gerais de argumentos (Fase 168)
- entrada mínima por stdin textual de processo externo em `--run` com `executar_com_entrada(comando, entrada) -> bombom`, sem shell implícito, sem sessão interativa, com uma única escrita textual de `verso` em stdin e retorno apenas do código de saída (Fase 165)
- pipe mínimo entre dois processos em `--run` com `pipeline_minimo(produtor, consumidor) -> bombom`, conectando apenas `stdout` do produtor ao `stdin` do consumidor, sem shell implícito, sem cadeia longa e retornando apenas o código de saída do consumidor (Fase 166)
- REPL mínimo auditável (`pink repl`) com reaproveitamento do pipeline real: cada linha vira o corpo temporário de `principal`, sem estado persistente entre linhas, sem multiline amplo e com `:quit`/`:sair` para saída explícita (Fase 167)
- captura mínima de stdout de processo externo em `--run` com `capturar_stdout(comando) -> verso`, sem shell implícito, sem argv rico, com UTF-8 estrito e retorno apenas do stdout textual (Fase 163)
- argv explícito mínimo em captura de stdout no `--run` com `capturar_stdout(comando, argv1) -> verso`, aceitando exatamente um argumento textual adicional sem shell implícito, sem quoting amplo e sem listas gerais de argumentos (Fase 169)
- captura mínima de stderr de processo externo em `--run` com `capturar_stderr(comando) -> verso`, sem shell implícito, sem argv rico, com UTF-8 estrito e retorno apenas do stderr textual (Fase 164)
- argv explícito mínimo em captura de stderr no `--run` com `capturar_stderr(comando, argv1) -> verso`, aceitando exatamente um argumento textual adicional sem shell implícito, sem quoting amplo e sem listas gerais de argumentos (Fase 170)
- norma visual oficial mínima da Pinker registrada em `docs/style.md`, definindo convenções de estilo para organização de imports, espaçamento de blocos, assinaturas de funções e tom de documentação, sem alterar a gramática ou o runtime (Fase 171)
- exportação mínima de `ninho` via `trazer` no sistema de módulos tipado (camada 1 conservadora): `ninho` de módulo importado passa a ser resolvível no arquivo consumidor para assinaturas e usos tipados mínimos já suportados, sem abrir `pub/priv`, exportação seletiva ou redesign amplo do sistema de módulos (Fase 144)

- exportação mínima de `apelido` via `trazer` no sistema de módulos tipado (camada 1 conservadora): `apelido` de módulo importado passa a ser resolvível no arquivo consumidor para declarações locais, assinaturas e cast tipado já suportado, sem abrir `pub/priv`, exportação seletiva ou redesign amplo (Fase 145)
- uso qualificado mínimo de tipo importado no sistema de módulos tipado (camada 1 conservadora): tipos exportados por módulo importado passam a aceitar referência qualificada em contexto tipado (`modulo.Tipo`) para declaração local, assinatura e cast tipado já suportado, mantendo recorte conservador sem `pub/priv`, sem reexportação e sem namespaces amplos (Fase 146)
- convenção documental mínima para imports e uso qualificado registrada em `docs/style.md`: `trazer` logo após `pacote`, um por linha, com módulo inteiro antes de símbolos pontuais quando coexistirem; preferir `modulo.Tipo` nos docs quando isso deixar a origem do tipo mais clara, sem criar regra nova de linguagem (Fase 174)
- array fixo operacional mínimo por valor no Bloco 13 (camada 1 conservadora): indexação `a[i]` para `a: [bombom; N]` em contexto real mínimo (`param/local`) sem heap, sem coleção dinâmica, sem métodos e sem sintaxe nova (Fase 147)
- escrita mínima por índice em array fixo `[bombom; N]` no Bloco 13 (camada 1 conservadora): `a[i] = valor` para `a: [bombom; N]` e índice `bombom`, complementando a leitura da Fase 147 sem heap, sem coleções dinâmicas e sem sintaxe nova (Fase 148)
- coleções dinâmicas mínimas no Bloco 13 em recorte conservador: `lista<bombom>` com criação explícita (`lista_bombom_criar()`), append mínimo (`lista_bombom_anexar(lista, v)`), leitura por índice (`lista_bombom_obter(lista, i)`), tamanho mínimo (`lista_bombom_tamanho(lista)`), escrita mínima por índice (`lista_bombom_definir(lista, i, v)`), remoção mínima do fim com retorno (`lista_bombom_tirar_ultimo(lista)`) e `mapa<verso,bombom>` mínimo com criação explícita (`mapa_verso_bombom_criar()`), definição por chave (`mapa_verso_bombom_definir(mapa, chave, valor)`), leitura por chave (`mapa_verso_bombom_obter(mapa, chave)`) e verificação de presença (`mapa_verso_bombom_tem(mapa, chave)`), sem abrir `lista<T>`/`mapa<K,V>` amplos, sem iteração confortável ampla ou API rica de coleção (Fases 149, 150, 151 e 152)
- iteração confortável mínima sobre `lista<bombom>` e `mapa<verso,bombom>` no Bloco 13 (camada 1 conservadora): `para cada item em lista { ... }` e `para cada chave em mapa { ... }` com variável de loop no corpo e lowering conservador por desdobramento; valor de mapa acessado via `mapa_verso_bombom_obter`; a Fase 155 corrige a iteração de mapa para usar cursor interno com snapshot de chaves, sem expor chave por índice como intrínseca pública; sem iteração genérica, sem pares chave/valor amplos e sem API ampla de iteradores (Fases 153, 154 e 155)
- aleatoriedade básica com semente explícita no Bloco 13 (camada 1 conservadora): `aleatorio_criar(semente)` cria um gerador mínimo reproduzível e `aleatorio_proximo(gerador)` produz o próximo `bombom` da sequência; mesma semente produz a mesma sequência, sem tempo do sistema, sem floats, sem distribuições ricas, sem shuffle e sem API criptográfica (Fase 156)
- formatação e dados estruturados mínimos no Bloco 14 (camada 1 conservadora): `formatar_verso(modelo, a[, b]) -> verso` monta texto com placeholders sequenciais `{}` e substituição controlada para `bombom` e `verso`; `ler_linha_csv_bombom(linha, sep) -> lista<bombom>` e `emitir_linha_csv_bombom(itens, sep) -> verso` abrem CSV mínimo de uma única linha com separador explícito e sem quoting/multiline; `ler_json_plano_bombom(json) -> mapa<verso,bombom>` e `emitir_json_plano_bombom(mapa) -> verso` abrem JSON mínimo de objeto plano com chaves textuais sem escape e valores `bombom`, em emissão determinística e auditável; `tempo_unix() -> bombom` e `formatar_tempo_unix(ts) -> verso` abrem o primeiro recorte temporal do projeto com timestamp Unix e formatação UTC fixa `YYYY-MM-DDTHH:MM:SSZ`; recorte pequeno, conservador e focado em integração simples (Fases 157, 158, 159 e 160)
- diretório atual mínimo em `--run` com `diretorio_atual()` retornando `verso` (Fase 95)
- introspecção mínima de caminho em `--run` com `caminho_existe(verso) -> logica` e `e_arquivo(verso) -> logica` (Fase 96)
- refinamento mínimo de caminho em `--run` com `e_diretorio(verso) -> logica` e `juntar_caminho(verso, verso) -> verso` (Fase 97)
- refinamento mínimo de arquivo em `--run` com `tamanho_arquivo(verso) -> bombom` e `e_vazio(verso) -> logica` (Fase 98)
- refinamento mínimo de mutação de filesystem em `--run` com `criar_diretorio(verso) -> nulo` e `remover_arquivo(verso) -> nulo` (Fase 99)
- refinamento mínimo complementar em `--run` com `remover_diretorio(verso) -> nulo` e leitura textual mínima `ler_verso_arquivo(handle) -> verso` (Fase 100)
- escrita textual mínima em `--run` com `escrever_verso(handle, verso) -> nulo` e criação mínima de arquivo com `criar_arquivo(verso) -> bombom` (Fase 101)
- truncamento mínimo de arquivo em `--run` com `truncar_arquivo(handle) -> nulo`, com observação explícita de pós-estado via `tamanho_arquivo`/`e_vazio` e releitura textual no mesmo handle (Fase 102)
- observação textual mínima em `--run` com `contem_verso(verso, verso) -> logica`, `comeca_com(verso, verso) -> logica`, `termina_com(verso, verso) -> logica` e `igual_verso(verso, verso) -> logica`, priorizando predicados simples para scripts sem abrir API textual ampla (Fase 104)
- saneamento textual mínimo em `--run` com `vazio_verso(verso) -> logica` (vazio exato) e `aparar_verso(verso) -> verso` (aparo de bordas), mantendo recorte pequeno e sem abrir API textual ampla (Fase 105)
- normalização mínima de caixa em `--run` com `minusculo_verso(verso) -> verso` e `maiusculo_verso(verso) -> verso`, mantendo recorte local e sem abrir casefolding/locale-aware/text API ampla (Fase 106)
- observação textual posicional mínima em `--run` com `indice_verso_em(verso, verso) -> bombom` (primeira ocorrência; retorna `18446744073709551615` quando ausente) e ergonomia mínima de presença com `nao_vazio_verso(verso) -> logica` (Fase 107)
- append textual mínimo em `--run` com `abrir_anexo(verso) -> bombom` e `anexar_verso(bombom, verso) -> nulo`, sem newline implícito e sem abrir modos ricos de arquivo (Fase 108)
- divisão textual mínima em `--run` com `dividir_verso_em(verso, verso, bombom) -> verso` (N-ésimo pedaço, base 0) e `dividir_verso_contar(verso, verso) -> bombom` (número de pedaços); separador simples, sem regex, sem coleção geral, resultado em tipos já existentes (Fase 137)
- substituição textual mínima em `--run` com `substituir_verso(verso, verso, verso) -> verso` (todas as ocorrências literais; padrão vazio rejeitado; `para` pode ser vazio para remoção); sem regex, sem flags, sem modos amplos (Fase 138)
- junção textual com separador em `--run` com `juntar_verso_com(verso, verso, verso) -> verso` (junta dois pedaços com separador explícito; encadeável para múltiplos pedaços; complementa `juntar_verso` já existente); sem coleções gerais, sem múltiplos modos (Fase 139)
- busca textual mínima em `--run` com `buscar_verso(verso, verso) -> bombom` (primeira ocorrência literal, com sentinela `18446744073709551615` quando ausente e rejeição explícita de padrão vazio no recorte mínimo) (Fase 140)
- leitura textual mínima direta por caminho em `--run` com `ler_arquivo_verso(verso) -> verso` e fallback ergonômico `arquivo_ou(verso, verso) -> verso`, sem streaming, sem escrita por caminho e sem API rica de handles (Fase 109)
- comando de projeto `pink build <arquivo.pink>` para gerar artefato textual `.s` em disco (padrão: `build/<arquivo>.s`)
- backend nativo real (subset externo montável) ampliado para múltiplos blocos, labels, salto incondicional (`jmp`), branch condicional mínimo (`br`) e loops reais mínimos (Fase 113), globais estáticas mínimas em `.rodata` (Fase 114), ABI mínima mais larga (Fase 115) com call direta de até 3 argumentos, compostos mínimos camada 1 (Fase 116) via parâmetro `seta<bombom>` + `deref_load` (`*ptr`), compostos mínimos camada 2 conservadora (Fase 117) com local `seta<bombom>` + offset explícito e compostos mínimos camada 3 conservadora (Fase 118) com `deref_store` homogêneo mínimo (`*ptr = valor`) e camada 4 conservadora (Fase 119) com consolidação auditável de par homogêneo mínimo (leituras/escritas coesas via `seta<bombom>` + offsets explícitos), além da abertura mínima da Fase 120 para `u32` em parâmetros/locais, da Fase 121 para `u64` em parâmetros/locais, da Fase 122 para `!=` mínima, da Fase 123 para `>` mínimo, da Fase 124 para `<=` mínimo e da Fase 125 para `>=` mínimo e da Fase 126 para `quebrar`/`continuar` em recorte mínimo de loop no caminho externo, da Fase 127 para aninhamento mínimo controlado em `sempre que` aninhado e da Fase 128 para composição mínima auditável até três níveis de `sempre que` com alvos distintos de `quebrar`/`continuar`, da Fase 129 para primeiro recorte heterogêneo mínimo de `ninho` no backend externo (leitura de campo `u32` via `seta<ninho>` + offset explícito, sem abrir composto amplo), da Fase 130 para camada 2 conservadora desse mesmo recorte (leitura de campo `u64` em `seta<ninho>` via offset explícito), da Fase 131 para escrita heterogênea mínima (`u32`/`u64`) e da Fase 132 para composição heterogênea mínima auditável no mesmo `ninho` (leitura+escrita `u32`/`u64` sem abrir sistema geral de campos/layout), da Fase 133 para abertura mínima de `virar` no backend externo (`u32 -> u64` explícito com origem em slot), da Fase 134 para camada 2 conservadora (`u64 -> u32` explícito no mesmo recorte por slot) e da Fase 135 para abertura mínima e condicional de `verso` no caminho externo (literal estático em `.rodata` + carga de endereço + tráfego opaco por slot/parâmetro, sem operações textuais gerais).
- chamadas diretas por nome
- checagem semântica de `principal`, retorno, mutabilidade, aridade e tipos
- AST textual estável
- AST JSON estável
- IR estruturada + validação interna
- CFG IR + validação interna
- seleção de instruções textual + validação
- alvo textual abstrato (máquina de pilha) + validação estrutural e disciplina de pilha
- backend textual pseudo-assembly + validacao interna
- proteção preventiva de recursão no runtime (`--run`) com limite interno de profundidade de chamadas
- metadata mínima de boot entry + linker script textual em modo `livre` na saída `--asm-s`
- editor/TUI oficial mínimo (`pink editor <arquivo.pink>`) com abertura de `.pink`, visualização textual em layout TUI simples (header + editor + painel de saída), edição mínima por comando (`:append`, `:set`, `:save`) e ação Pinker real no painel (`:tokens`, `:ast`)

## O que não faz
- codegen nativo real
- backend nativo pleno
- LLVM / Cranelift
- otimizações grandes
- FFI, enums, generics, traits
- operações completas de ponteiro (aritmética além do subset mínimo atual, como `n + ptr`, `ptr - ptr`), acesso completo via ponteiro (`seta<T>`), escrita em campo/index, layout físico/ABI
- acesso operacional de campo de `ninho` além do subset atual (ex.: base por valor `p.campo`, escrita de campo, campos não escalares)
- indexação operacional de arrays além do subset atual (ex.: elementos não `bombom` e operações de array fora do recorte conservador)
- coleções dinâmicas além do recorte mínimo atual (`lista<bombom>` + `mapa<verso,bombom>`): seguem fora `lista<T>`/`mapa<K,V>` amplos, remoção arbitrária por chave, ordenação, insert/erase gerais e iteração confortável ampla (há recorte mínimo de `para cada` sobre `lista<bombom>` e `mapa<verso,bombom>` nas Fases 153, 154 e 155)
- aleatoriedade além do recorte mínimo atual: seguem fora distribuição rica, geração de `float`, intervalo dedicado, shuffle, escolha aleatória sobre coleção, tokens/UUIDs, hash aleatório e qualquer API criptográfica (há apenas `aleatorio_criar(semente)` + `aleatorio_proximo(gerador)` na Fase 156)
- leitura indireta além do subset mínimo atual (`*p` apenas para `seta<bombom>` com endereçamento abstrato de globals escalares no runtime)
- escrita indireta além do subset mínimo atual (`*p = v` apenas para `seta<bombom>` com endereçamento abstrato de globals escalares já mapeadas no runtime)
- semântica completa de `fragil` em runtime/backend (há apenas efeito operacional mínimo em acessos indiretos no subset `fragil seta<bombom>`, sem MMIO/fences/ordenação de memória)
- lowering operacional de `virar` fora do subset atual (`--run` executa inteiro->inteiro e `bombom <-> seta<bombom>`; no backend externo `--asm-s` há recorte mínimo explícito `u32 -> u64` e `u64 -> u32` com origem em slot local/parâmetro; demais casts continuam rejeitados)
- lowering operacional de inline asm em CFG/Machine/runtime (`--check`/`--ir` aceitam o subset atual; `--cfg-ir`/`--run` ainda não executam `sussurro`)
- operações de texto em `verso` além do recorte mínimo atual (ex.: slicing, indexação negativa e formatação rica) ainda fora do subset operacional
- API rica de arquivo (múltiplos modos gerais, streaming/diretórios e variações além de `abrir_anexo` + `anexar_verso`)
- metadados de arquivo além do recorte mínimo atual (`tamanho_arquivo` e `e_vazio`)
- mutação de filesystem além do recorte mínimo atual (`criar_diretorio` simples, `remover_arquivo` simples e `remover_diretorio` simples sem recursão)
- mutação/listagem ampla de ambiente de processo (apenas leitura mínima com fallback)
- mudança de diretório e API rica de paths
- introspecção de caminho além do recorte mínimo atual (`caminho_existe`, `e_arquivo`, `e_diretorio` e `juntar_caminho`)
- leitura de arquivo além do recorte mínimo da Fase 86 (apenas conteúdo inteiro `bombom` via `ler_arquivo`)
- leitura textual de arquivo além do recorte mínimo da Fase 100 (`ler_verso_arquivo` retorna conteúdo completo do handle, sem streaming/append/encoding avançado)
- escrita textual além do recorte mínimo da Fase 101 (`escrever_verso` sobrescreve conteúdo inteiro do handle, sem append/streaming/escrita por linha)
- truncamento além do recorte mínimo da Fase 102 (sem truncamento por caminho, sem streaming e sem modos ricos de arquivo)
- operações textuais além do recorte mínimo das Fases 137–140 (sem regex, sem busca de múltiplas ocorrências, sem busca reversa, sem `join` de coleção arbitrária, sem coleções gerais, sem biblioteca textual ampla)
- formatação avançada de saída
- freestanding/no-std operacional real (`livre;` é marca semântica de intenção, não runtime bare-metal executável)
- editor completo/IDE ampla (sem LSP/autocomplete, sem árvore de símbolos, sem watch sofisticado, sem terminal geral embutido)

## Build e testes
```bash
cargo build
cargo test
```

## CI + MSRV
- CI em `.github/workflows/ci.yml` rodando: `cargo build --locked`, `cargo check --locked`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --locked` e `cargo doc --no-deps -D warnings`.
- MSRV adotada: **Rust 1.78.0** (fixada em `rust-toolchain.toml`).

### Comandos locais equivalentes ao CI
```bash
cargo build --locked
cargo check --locked
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

## Binários do projeto
- `pink`: binário principal da CLI da linguagem.
- `pinker_mcp` (histórico): binário MCP mínimo descontinuado e removido por segurança; não faz parte da operação atual.

## Uso
```bash
cargo run --bin pink -- examples/principal_valida.pink
cargo run --bin pink -- repl
cargo run --bin pink -- --ir examples/ir_if_else.pink
cargo run --bin pink -- --cfg-ir examples/cfg_if_else.pink
cargo run --bin pink -- --selected examples/selected_if_else.pink
cargo run --bin pink -- --machine examples/machine_if_else.pink
cargo run --bin pink -- --machine examples/machine_stack_if_call.pink
cargo run --bin pink -- --pseudo-asm examples/emit_if_else.pink
cargo run --bin pink -- --asm-s examples/emit_if_else.pink
cargo run --bin pink -- --run examples/run_soma.pink
cargo run --bin pink -- --run examples/run_chamada.pink
cargo run --bin pink -- --run examples/run_sempre_que.pink
cargo run --bin pink -- --run examples/run_quebrar.pink
cargo run --bin pink -- --run examples/run_continuar.pink
cargo run --bin pink -- --run examples/run_global.pink
cargo run --bin pink -- --run examples/run_unsigned_basico.pink
cargo run --bin pink -- --run examples/run_signed_basico.pink
cargo run --bin pink -- --run examples/run_alias_tipo_basico.pink
cargo run --bin pink -- --run examples/fase64_falar_signed.pink
cargo run --bin pink -- --run examples/fase66_deref_leitura_valido.pink
cargo run --bin pink -- --run examples/fase67_escrita_indireta_valida.pink
cargo run --bin pink -- --run examples/fase68_ptr_aritmetica_valida.pink
cargo run --bin pink -- --run examples/fase68_ptr_aritmetica_leitura_valida.pink
cargo run --bin pink -- --run examples/fase69_ninho_campo_operacional_valido.pink
cargo run --bin pink -- --run examples/fase70_indexacao_array_operacional_valido.pink
cargo run --bin pink -- --run examples/fase71_cast_memoria_valido.pink
cargo run --bin pink -- --run examples/fase72_fragil_operacional_minimo_valido.pink
printf '41\n' | cargo run --bin pink -- --run examples/fase85_ouvir_bombom_valido.pink
cargo run --bin pink -- --run examples/fase86_arquivo_leitura_minima_valido.pink
cargo run --bin pink -- --run examples/fase87_arquivo_escrita_minima_valido.pink
cargo run --bin pink -- --run examples/fase88_verso_operacional_minimo_valido.pink
cargo run --bin pink -- --run examples/fase89_verso_operacoes_minimas_valido.pink
cargo run --bin pink -- --run examples/fase90_verso_indexacao_minima_valido.pink
cargo run --bin pink -- --run examples/fase91_falar_multiplos_argumentos_valido.pink
cargo run --bin pink -- --run examples/fase92_tooling_base_argumento_status_valido.pink -- Pinker
cargo run --bin pink -- --run examples/fase93_argv_ergonomia_minima_valido.pink -- A beta
cargo run --bin pink -- --run examples/fase94_argumento_ou_fallback_minimo_valido.pink -- Pinker
cargo run --bin pink -- --run examples/fase141_argumentos_nomeados_minimos_valido.pink -- --saida out.txt
cargo run --bin pink -- --run examples/fase141_argumentos_nomeados_minimos_valido.pink -- --saida=out.txt --modo=rapido
cargo run --bin pink -- --run examples/fase142_flags_booleanas_minimas_valido.pink -- --quiet
cargo run --bin pink -- --run examples/fase142_flags_booleanas_minimas_valido.pink -- --quiet --saida out.txt
cargo run --bin pink -- --run examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink -- --saida out.txt
env PINKER_FASE143_SAIDA=env.txt cargo run --bin pink -- --run examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink
env -u PINKER_FASE143_SAIDA cargo run --bin pink -- --run examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink
cargo run --bin pink -- --run examples/fase95_ambiente_processo_minimo_valido.pink
cargo run --bin pink -- --run examples/fase95_diretorio_atual_minimo_valido.pink
cargo run --bin pink -- --run examples/fase95_argumento_ou_ambiente_ou_valido.pink -- Pinker
cargo run --bin pink -- --run examples/fase96_introspeccao_caminho_minima_valido.pink
cargo run --bin pink -- --run examples/fase97_refinamento_caminho_minimo_valido.pink
cargo run --bin pink -- --run examples/fase98_refinamento_arquivo_minimo_valido.pink
echo 7 > /tmp/pinker_fase99_temp.txt
cargo run --bin pink -- --run examples/fase99_refinamento_diretorio_arquivo_minimo_valido.pink -- fase99_saida_local /tmp/pinker_fase99_temp.txt
cargo run --bin pink -- --run examples/fase100_refinamento_diretorio_texto_minimo_valido.pink -- fase100_saida_local README.md
cargo run --bin pink -- --run examples/fase101_escrita_textual_minima_arquivo_valido.pink -- /tmp fase101_saida.txt "texto fase101"
cargo run --bin pink -- --run examples/fase102_truncamento_minimo_arquivo_valido.pink -- /tmp fase102_saida.txt
cargo run --bin pink -- --run examples/fase103_observacao_textual_minima_valido.pink -- /tmp fase103_entrada.txt
cargo run --bin pink -- --run examples/fase104_observacao_textual_complementar_minima_valido.pink -- /tmp fase104_entrada.txt
cargo run --bin pink -- --run examples/fase105_saneamento_textual_minimo_valido.pink -- /tmp fase105_entrada.txt
cargo run --bin pink -- --run examples/fase106_normalizacao_minima_caixa_valido.pink -- "PiNkEr V0"
cargo run --bin pink -- --run examples/fase107_observacao_textual_posicional_minima_valido.pink -- "   pinker v0   "
cargo run --bin pink -- --run examples/fase108_append_textual_minimo_valido.pink -- /tmp fase108_saida.txt
cargo run --bin pink -- --run examples/fase109_leitura_textual_direta_por_caminho_valido.pink -- /tmp/pinker_fase109_saida.txt
printf 'linha110\n' | cargo run --bin pink -- --run examples/fase110_entrada_textual_minima_valida.pink
cargo run --bin pink -- --asm-s examples/fase73_backend_externo_locais_aritmetica_valido.pink
cargo run --bin pink -- --check examples/fase74_backend_externo_call_minimo_valido.pink
cargo run --bin pink -- --asm-s examples/fase75_backend_externo_frame_registradores_valido.pink
cargo run --bin pink -- --asm-s examples/fase75_backend_externo_parametro_nao_bombom_invalido.pink
cargo run --bin pink -- --asm-s examples/fase76_backend_externo_multiplos_parametros_valido.pink
cargo run --bin pink -- --asm-s examples/fase77_backend_externo_memoria_frame_valido.pink
cargo run --bin pink -- --asm-s examples/fase78_backend_externo_composicao_interprocedural_valido.pink
cargo run --bin pink -- --asm-s examples/fase79_backend_externo_programa_linear_maior_valido.pink
cargo run --bin pink -- --asm-s examples/fase80_backend_externo_cobertura_linear_ampla_valido.pink
cargo run --bin pink -- --asm-s examples/fase81_backend_externo_recusa_explicita_tres_parametros_invalido.pink
cargo run --bin pink -- --asm-s examples/fase112_branch_condicional_minimo_valido.pink
cargo run --bin pink -- --asm-s examples/fase118_compostos_minimos_camada3_valida.pink
cargo run --bin pink -- --asm-s examples/fase120_tipos_inteiros_mais_largos_valido.pink
cargo run --bin pink -- --asm-s examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink
cargo run --bin pink -- --asm-s examples/fase122_comparacoes_ampliadas_camada1_valido.pink
cargo run --bin pink -- --asm-s examples/fase123_comparacoes_ampliadas_camada2_valido.pink
cargo run --bin pink -- --asm-s examples/fase124_comparacoes_ampliadas_camada3_valido.pink
cargo run --bin pink -- --asm-s examples/fase125_comparacoes_ampliadas_camada4_valido.pink
cargo run --bin pink -- build examples/fase126_quebrar_continuar_camada1_valido.pink
cargo run --bin pink -- build examples/fase127_quebrar_continuar_camada2_valido.pink
cargo run --bin pink -- build examples/fase128_quebrar_continuar_camada3_valido.pink
cargo run --bin pink -- --asm-s examples/fase129_ninho_heterogeneo_camada1_valido.pink
cargo run --bin pink -- --asm-s examples/fase130_ninho_heterogeneo_camada2_valido.pink
cargo run --bin pink -- --asm-s examples/fase132_ninho_heterogeneo_camada4_valido.pink
cargo run --bin pink -- --asm-s examples/fase133_virar_camada1_valido.pink
cargo run --bin pink -- --asm-s examples/fase134_virar_camada2_valido.pink
cargo run --bin pink -- --asm-s examples/fase135_verso_camada1_valido.pink
cargo run --bin pink -- --run examples/fase138_replace_camada1_valido.pink
cargo run --bin pink -- --run examples/fase140_busca_textual_camada1_valido.pink
cargo run --bin pink -- --run examples/fase160_tempo_basico_timestamp_valido.pink
cargo run --bin pink -- --run examples/fase160_tempo_basico_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase161_processo_externo_minimo_valido.pink -- target/debug/pinker_fase162_exit0
cargo run --bin pink -- --run examples/fase161_processo_externo_fluxo_composto_valido.pink -- target/debug/pinker_fase162_exit0 target/debug/pinker_fase162_exit1
cargo run --bin pink -- --run examples/fase163_captura_stdout_minima_valido.pink -- target/debug/pinker_fase163_stdout_ok
cargo run --bin pink -- --run examples/fase163_captura_stdout_fluxo_composto_valido.pink -- target/debug/pinker_fase163_stdout_ok
cargo run --bin pink -- --run examples/fase169_captura_stdout_argv_explicito_minimo_valido.pink -- target/debug/pinker_fase163_stdout_ok
cargo run --bin pink -- --run examples/fase169_captura_stdout_argv_explicito_fluxo_composto_valido.pink -- target/debug/pinker_fase163_stdout_ok
cargo run --bin pink -- --run examples/fase164_captura_stderr_minima_valido.pink -- target/debug/pinker_fase164_stderr_ok
cargo run --bin pink -- --run examples/fase164_captura_stderr_fluxo_composto_valido.pink -- target/debug/pinker_fase164_stderr_ok
cargo run --bin pink -- --run examples/fase170_captura_stderr_argv_explicito_minimo_valido.pink -- target/debug/pinker_fase164_stderr_ok
cargo run --bin pink -- --run examples/fase170_captura_stderr_argv_explicito_fluxo_composto_valido.pink -- target/debug/pinker_fase164_stderr_ok
cargo run --bin pink -- --run examples/fase165_stdin_textual_minimo_valido.pink -- target/debug/pinker_fase165_stdin_ok
cargo run --bin pink -- --run examples/fase165_stdin_textual_fluxo_composto_valido.pink -- target/debug/pinker_fase165_stdin_ok
cargo run --bin pink -- --run examples/fase166_pipe_minimo_valido.pink -- target/debug/pinker_fase166_pipe_produtor target/debug/pinker_fase165_stdin_ok
cargo run --bin pink -- --run examples/fase166_pipe_minimo_fluxo_composto_valido.pink -- target/debug/pinker_fase166_pipe_produtor target/debug/pinker_fase165_stdin_ok
cargo run --bin pink -- --run examples/fase168_argv_explicito_minimo_valido.pink -- target/debug/pinker_fase168_argv_um
cargo run --bin pink -- --run examples/fase168_argv_explicito_fluxo_composto_valido.pink -- target/debug/pinker_fase168_argv_um
cargo run --bin pink -- --asm-s examples/fase84_backend_externo_recusa_explicita_sempre_que_invalido.pink
cargo run --bin pink -- --check examples/fase76_backend_externo_tres_args_invalido.pink
cargo run --bin pink -- --check examples/mut_falho.pink
cargo run --bin pink -- --check examples/check_quebrar_fora_loop.pink
cargo run --bin pink -- --check examples/check_continuar_fora_loop.pink
cargo run --bin pink -- --check examples/check_campo_valido.pink
cargo run --bin pink -- --check examples/check_indexacao_valida.pink
cargo run --bin pink -- --check examples/check_indexacao_indice_nao_inteiro.pink
cargo run --bin pink -- --check examples/check_cast_inteiro_valido.pink
cargo run --bin pink -- --check examples/fase71_cast_memoria_invalido.pink
cargo run --bin pink -- --check examples/fase72_fragil_operacional_minimo_invalido.pink
cargo run --bin pink -- --check examples/check_cast_invalido_logica.pink
cargo run --bin pink -- --check examples/check_peso_alinhamento_escalar.pink
cargo run --bin pink -- --check examples/check_peso_alinhamento_array.pink
cargo run --bin pink -- --check examples/check_peso_alinhamento_ninho.pink
cargo run --bin pink -- --check examples/check_peso_tipo_inexistente.pink
cargo run --bin pink -- --check examples/check_volatile_valido.pink
cargo run --bin pink -- --check examples/check_volatile_invalido.pink
cargo run --bin pink -- --check examples/check_inline_asm_valido.pink
cargo run --bin pink -- --check examples/check_inline_asm_multilinha.pink
cargo run --bin pink -- --check examples/check_inline_asm_invalido_vazio.pink
cargo run --bin pink -- --check examples/check_freestanding_valido.pink
cargo run --bin pink -- --check examples/check_freestanding_invalido_fora_topo.pink
cargo run --bin pink -- --check examples/check_boot_entry_livre_valido.pink
cargo run --bin pink -- --check examples/check_boot_entry_livre_sem_principal.pink
cargo run --bin pink -- --check examples/check_kernel_minimo_fase59_valido.pink
cargo run --bin pink -- --check examples/fase61_verso_valido.pink
cargo run --bin pink -- --check examples/fase66_deref_seta_u8_invalido.pink
cargo run --bin pink -- --check examples/fase67_escrita_indireta_seta_u8_invalida.pink
cargo run --bin pink -- --check examples/fase68_ptr_aritmetica_invalida.pink
cargo run --bin pink -- --run examples/fase69_ninho_campo_operacional_invalido.pink
cargo run --bin pink -- --run examples/fase147_array_fixo_operacional_minimo_valido.pink
cargo run --bin pink -- --run examples/fase147_array_fixo_operacional_minimo_invalido.pink
cargo run --bin pink -- --run examples/fase148_array_fixo_escrita_indice_minima_valido.pink
cargo run --bin pink -- --run examples/fase149_lista_minima_bombom_valido.pink
cargo run --bin pink -- --run examples/fase149_lista_minima_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase149_lista_minima_bombom_homogenea_invalido.pink
cargo run --bin pink -- --check examples/fase150_lista_bombom_definir_minimo_valido.pink
cargo run --bin pink -- --run examples/fase150_lista_bombom_definir_minimo_valido.pink
cargo run --bin pink -- --check examples/fase150_lista_bombom_definir_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase150_lista_bombom_definir_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase151_lista_bombom_tirar_ultimo_minimo_valido.pink
cargo run --bin pink -- --run examples/fase151_lista_bombom_tirar_ultimo_minimo_valido.pink
cargo run --bin pink -- --check examples/fase151_lista_bombom_tirar_ultimo_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase151_lista_bombom_tirar_ultimo_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase152_mapa_verso_bombom_minimo_valido.pink
cargo run --bin pink -- --run examples/fase152_mapa_verso_bombom_minimo_valido.pink
cargo run --bin pink -- --check examples/fase152_mapa_verso_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase152_mapa_verso_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase153_iteracao_lista_bombom_minima_valido.pink
cargo run --bin pink -- --run examples/fase153_iteracao_lista_bombom_minima_valido.pink
cargo run --bin pink -- --check examples/fase153_iteracao_lista_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase153_iteracao_lista_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase154_iteracao_mapa_verso_bombom_minima_valido.pink
cargo run --bin pink -- --run examples/fase154_iteracao_mapa_verso_bombom_minima_valido.pink
cargo run --bin pink -- --check examples/fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase156_aleatoriedade_basica_semente_valido.pink
cargo run --bin pink -- --run examples/fase156_aleatoriedade_basica_semente_valido.pink
cargo run --bin pink -- --check examples/fase156_aleatoriedade_basica_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase156_aleatoriedade_basica_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase157_formatacao_simples_saida_valido.pink
cargo run --bin pink -- --run examples/fase157_formatacao_simples_saida_valido.pink
cargo run --bin pink -- --check examples/fase157_formatacao_simples_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase157_formatacao_simples_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase158_csv_minimo_valido.pink
cargo run --bin pink -- --run examples/fase158_csv_minimo_valido.pink
cargo run --bin pink -- --check examples/fase158_csv_minimo_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase158_csv_minimo_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase159_json_basico_valido.pink
cargo run --bin pink -- --run examples/fase159_json_basico_valido.pink
cargo run --bin pink -- --check examples/fase159_json_basico_fluxo_composto_valido.pink
cargo run --bin pink -- --run examples/fase159_json_basico_fluxo_composto_valido.pink
cargo run --bin pink -- --check examples/fase148_array_fixo_escrita_indice_elemento_nao_bombom_invalido.pink
cargo run --bin pink -- --cfg-ir examples/fase61_verso_cfg_ir_invalido.pink
cargo run --bin pink -- --run examples/fase60_modulos_valido.pink
cargo run --bin pink -- --check examples/fase60_modulo_ausente.pink
cargo run --bin pink -- --check examples/fase60_simbolo_ausente.pink
cargo run --bin pink -- --run examples/fase62_falar_inteiro.pink
cargo run --bin pink -- --run examples/fase62_falar_logica.pink
cargo run --bin pink -- --run examples/fase62_falar_verso.pink
cargo run --bin pink -- --run examples/fase62_falar_expr.pink
cargo run --bin pink -- build examples/emit_if_else.pink
cargo run --bin pink -- build --out-dir saida examples/fase60_modulos_valido.pink
```

## Modos da CLI
- `build <arquivo.pink>`: executa pipeline de build e grava artefato `.s` em disco (opcional `--out-dir <dir>`, padrão `build/`)
- `--ir`: IR estruturada (alto nível)
- `--cfg-ir`: CFG IR (blocos, `br`, `jmp`, `ret`)
- `--selected`: camada de seleção de instruções textual (`isel` + `term`)
- `--machine`: alvo textual abstrato de máquina de pilha (`vm` + `term`)
- `--pseudo-asm`: backend textual normalizado final (`ins`/`term`)
- `--asm-s`: backend textual `.s` com ABI textual mínima interna (derivado de `--selected`, sem ABI/registradores finais de plataforma)
- `--run`: interpreta a Machine validada e executa `principal` (suporta `-- <args...>` para repasse de argv posicional em `argumento`, `tem_argumento`, `quantos_argumentos` e `argumento_ou`, além do recorte nomeado mínimo canônico `tem_chave`/`pedir_argumento` com compatibilidade temporária para `tem_argumento_nomeado`/`argumento_nomeado_ou`, do recorte de flags booleanas mínimas `tem_flag` com detecção de presença literal sem consumo de valor e da prioridade mínima `buscar_contexto(chave_arg, chave_env, padrao)` com compatibilidade temporária para `argumento_nomeado_ou_ambiente_ou`, sempre em `CLI -> ambiente -> padrão`; inclui também leitura mínima de ambiente com `ambiente_ou`, diretório atual via `diretorio_atual`, introspecção/composição mínima de caminho com `caminho_existe`/`e_arquivo`/`e_diretorio`/`juntar_caminho`, metadados mínimos de arquivo com `tamanho_arquivo`/`e_vazio`, leitura textual mínima via `ler_verso_arquivo(handle)` e leitura textual direta por caminho via `ler_arquivo_verso(verso)`/`arquivo_ou(verso, verso)`, escrita textual mínima via `escrever_verso(handle, verso)`/`criar_arquivo(verso)`, truncamento mínimo via `truncar_arquivo(handle)`, observação textual mínima com `contem_verso`/`comeca_com`/`termina_com`/`igual_verso`, mutação mínima controlada com `criar_diretorio`/`remover_arquivo`/`remover_diretorio`, execução mínima de processo externo com `executar_processo(verso) -> bombom` e `executar_processo(verso, verso) -> bombom` para exatamente um `argv1` explícito, envio mínimo de stdin textual com `executar_com_entrada(verso, verso) -> bombom`, pipe mínimo entre dois processos com `pipeline_minimo(verso, verso) -> bombom`, captura mínima de stdout com `capturar_stdout(verso) -> verso` e `capturar_stdout(verso, verso) -> verso` para exatamente um `argv1` explícito e captura mínima de stderr com `capturar_stderr(verso) -> verso` e `capturar_stderr(verso, verso) -> verso` para exatamente um `argv1` explícito, sempre sem shell implícito, sem argv amplo, sem redirecionamento rico, sem cadeia longa de pipes, sem sessão interativa e sem API ampla de subprocesso)

## Pipeline de backend textual
`--pseudo-asm` executa:
semântica → IR estruturada → validação da IR estruturada → CFG IR → validação da CFG IR → seleção de instruções → validação da seleção → máquina abstrata → validação da máquina → backend textual → validação do backend textual → impressão.

`--run` executa:
semântica → IR estruturada → validação IR → CFG IR → validação CFG IR → seleção → validação seleção → Machine → validação Machine → interpretação.

Se qualquer camada intermediária for inválida, a emissão falha e nada é impresso.

`--asm-s` executa:
semântica → IR estruturada → validação IR → CFG IR → validação CFG IR → seleção de instruções → validação da seleção → emissão textual `.s` com ABI mínima.

`--asm-s` cobre o subset escalar (`bombom`, `u8..u64`, `i8..i64`, `logica`, `nulo`) com ABI textual mínima interna (símbolo exportado, `@argN`, `@ret`, prólogo/epílogo textuais). Tipos ainda não suportados seguem falhando de forma clara (ex.: `seta`, `ninho`, arrays).

Existe também integração externa **experimental e mínima** para Linux x86_64 via testes (`cc`/`gcc`/`clang`). O subset externo montável atual suporta:
- `principal() -> bombom` com variáveis locais `bombom`, atribuição local e aritmética escalar linear (`+`, `-`, `*`);
- chamadas diretas com **até 3 argumentos** no recorte conservador (com cobertura consolidada de `bombom` e aberturas mínimas para `u32`/`u64` em params/locals nas Fases 120–121), com convenção concreta mínima `%rdi/%rsi/%rdx` e retorno em `%rax`;
- frame mínimo explícito por função: `%rbp`, slots lineares para parâmetros/locais/temporários, `%r10` como temporário volátil de binárias;
- load/store em slots de frame via `%rbp` (`movq -off(%rbp), %reg` / `movq %reg, -off(%rbp)`);
- composição linear interprocedural (encadeamento de chamadas diretas em múltiplos níveis no mesmo executável);
- múltiplos blocos por função com labels nomeadas, `jmp`, branch condicional mínimo (`cmp` + `jcc`), loops reais mínimos e globais estáticas mínimas em `.rodata` no backend externo (Fases 113/114).

Fora do subset externo montável atual:
- sem loops amplos (`sempre que`) além do recorte mínimo da Fase 113;
- sem memória indireta geral/ponteiros;
- sem sistema global rico (apenas `eterno` literal `bombom`/`logica` em `.rodata`), sem 3+ parâmetros, sem tipos de parâmetro fora de `bombom`/`u32`/`u64`/`seta<bombom>`;
- sem recursão externa e sem ABI completa de plataforma/register allocation amplo.

Recusas explícitas e auditáveis:
- 3+ parâmetros por função/call → rejeitado com diagnóstico explícito;
- loops (`sempre que`) fora do recorte mínimo (`==`, `!=`, `<`, `>`, `<=` e `>=`) e comparações adicionais além desse recorte → rejeitado com diagnóstico explícito.

Fluxo experimental reproduzível:
```bash
cargo test --test backend_s_external_toolchain_tests -- --nocapture
```
Se não houver toolchain C no ambiente, o teste de fluxo real é pulado sem quebrar a suíte.

Fronteira auditável atual do subset externo (`--asm-s` montável):

| Caso | Situação | Evidência auditável mínima |
|---|---|---|
| `principal() -> bombom` com locals `bombom` + aritmética linear | garantido | exemplo `fase73_backend_externo_locais_aritmetica_valido` + teste externo |
| chamadas diretas com até 2 parâmetros `bombom` | garantido | exemplos `fase76`/`fase78`/`fase80` + testes externos |
| memória mínima de frame via `%rbp` (load/store em slots) | garantido | exemplo `fase77_backend_externo_memoria_frame_valido` + teste externo |
| múltiplos blocos + labels + `jmp` incondicional | garantido | exemplo `fase111_blocos_labels_salto_incondicional_valido` + testes externos |
| branch condicional mínimo com `==` + `cmp`/`jcc` | garantido | exemplo `fase112_branch_condicional_minimo_valido` + testes externos |
| loops reais mínimos com condição relacional não assinada no recorte `==`, `!=`, `<`, `>`, `<=` e `>=` | garantido | exemplo `fase113_loops_reais_minimos_validos` + testes externos |
| global estática mínima em `.rodata` com leitura por símbolo | garantido | exemplo `fase114_globais_minimas_rodata_base_valido` + testes externos |
| 3+ parâmetros por função/call | rejeitado explicitamente | exemplo `fase81_backend_externo_recusa_explicita_tres_parametros_invalido` + testes negativos |
| loops/condições fora do recorte mínimo (`==`, `!=`, `<`, `>`, `<=`, `>=`) | rejeitado explicitamente | exemplo `fase113_loop_condicao_invalida_invalido` + testes negativos |

`--check` continua restrito à validação semântica (não executa lowering IR/CFG nem emissão textual).

Em unidade com `livre;`, `--asm-s` emite metadata de boot entry textual mínima (`boot.entry principal -> _start`), linker script textual mínimo (`ENTRY(_start)` + seções básicas) e stub `_start` global chamando `principal` e entrando em loop de parada. Isso é representação/preparação textual: não gera kernel bootável real, não integra GRUB/QEMU e não substitui o fluxo hospedado.

## Validação da Machine (sanity check de pilha)
A camada `--machine` agora valida:
- underflow de pilha em instruções/terminadores (`neg`, binárias, `call`, `call_void`, `br_true`)
- consistência de altura de pilha entre predecessores de um bloco
- tipo esperado no topo para `br_true` (condição lógica)
- `ret` com exatamente um valor disponível
- compatibilidade de tipo no `ret` com o retorno da função quando inferível
- aproveitamento de tipos de `params`/`locals` para reduzir `Unknown` em `load_slot`/`store_slot`
- `ret_void` com pilha vazia
- slots válidos por função (`params`, `locals` e temporários `%tN`)

Se a validação estrutural ou de pilha falhar, `--machine` retorna erro e não imprime saída parcial.

Limites atuais (adiado): a tipagem na Machine continua leve/local (sem inferência global pesada entre blocos).

## O que essas camadas representam
- `--cfg-ir`: controle de fluxo explícito próximo do lowering
- `--selected`: instruções selecionadas e terminadores já disciplinados
- `--machine`: alvo textual abstrato de execução (pilha + controle de fluxo)
- `--pseudo-asm`: formato textual final estável para auditoria e golden tests

## O que ainda não representam
- não são assembly real de CPU
- não são backend executável
- não fazem otimizações ou alocação de registradores

## REPL mínimo

```text
$ cargo run --bin pink -- repl
=== Pinker REPL ===
Fase 167: cada linha vira o corpo temporário de `principal`; sem estado entre linhas.
Use `falar(...)` para inspecionar saída, `mimo ...;` para retorno explícito e `:quit` para sair.
pinker> nova a: bombom = 40; falar(a + 2);
42
ok
pinker> mimo 7;
=> 7
pinker> :quit
Encerrando REPL Pinker.
```

Limites atuais do REPL: sem estado persistente entre linhas, sem multiline amplo, sem autocomplete, sem histórico sofisticado e sem integração de shell rica.

## Documentação do projeto

A navegação documental principal está em `docs/atlas.md`, com arquitetura dual explícita:

- hemisfério **Engine** (factual/operacional);
- hemisfério **Rosa** (identitário/lexical/visionário);
- documento-ponte para manter conversa entre os dois lados.

Referências centrais:

- `docs/atlas.md` — arquivo mestre de navegação.
- `MANUAL.md` — manual de uso da linguagem no estado implementado.
- `docs/roadmap.md` — trilha ativa oficial.
- `docs/history.md` — crônica histórica oficial.
- `docs/rosa.md` — manifesto conceitual estruturado do hemisfério Rosa.
- `docs/vocabulario.md` — arquitetura lexical canônica.
- `docs/ponte_engine_rosa.md` — ponte explícita entre factual e visão.
- `docs/future.md` e `docs/parallel.md` — acervos de apoio (técnico e visionário).

## Licença
[MIT](LICENSE)
