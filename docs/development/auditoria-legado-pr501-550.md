# Auditoria do legado da Pinker — era dos PRs #501–550

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Décimo primeiro documento da série. Cobre os **PRs #501 a #550**, mergeados entre
**22 e 30 de agosto de 2026**.

---

## 1. A era dos módulos e das identidades

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 12.305 | 6,08% |
| `src/` | 4.697 | 5,15% |

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| 99,3% | 271 / 273 | `src/module_graph.rs` |
| 82,3% | 1.692 / 2.055 | `src/module_resolve.rs` |
| 74,7% | 511 / 684 | `src/intrinsics/identity.rs` |
| 70,2% | 617 / 879 | `src/intrinsics/public_surface.rs` |

O trabalho: composição de módulos, migração de módulos, imports múltiplos,
identidade canônica de processo, autoridade de intrínsecas, paridade nativa,
quarentena e recuperação, pilha não executável.

---

## 2. Nenhum achado novo de dívida — e um contraste que vale registrar

Como a era anterior, esta não produz achado próprio. O que ela produz é o
**contraste** que fecha o achado L-24.

L-24 registrou 26 arquivos de teste rotulados por taxonomias ausentes de `docs/`
(`part_b`…`part_g`, `u1_f03`…`u4_f10`, `d4`…`d13`). Esta era mudou a convenção:

```
tests/issue_505_module_migration_tests.rs      785 linhas
tests/issue_514_module_composition_tests.rs  2.077 linhas
tests/issue522_native_parity_tests.rs          560 linhas
tests/issue525_identidade_canonica_processo_tests.rs   486
tests/issue_533_multi_import_tests.rs          855 linhas
```

**Cada um desses nomes é rastreável.** Um número de issue leva a uma discussão, a
uma decisão e a um PR. Não depende de `docs/` conter o rótulo, porque o rótulo é
um endereço real do GitHub.

Quinze arquivos de teste do repositório usam essa convenção hoje. Ela é
estritamente melhor do que `part_g` ou `d8`, e não custou nada implantar — foi
só parar de inventar sigla.

**Recomendação derivada, e é a mais barata de toda a série:** adotar
`issue_NNN_` como convenção única para teste vinculado a um item de trabalho, e
registrar isso em `CONTRIBUTING.md`. Não conserta os 26 arquivos antigos — a
rodada documental de L-24 é que faz isso — mas impede o 27º.

---

## 3. Registro acumulado

Sem acréscimo de achado. A série segue com L-01 a L-24.
