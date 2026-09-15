# Auditoria do legado da Pinker — era dos PRs #451–500

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Décimo documento da série. Cobre os **PRs #451 a #500**, mergeados entre **10 e
22 de agosto de 2026**.

Pré-requisitos: os nove documentos anteriores da série.

---

## 1. A era dos contratos

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 30.427 | 15,04% |
| `src/` | 10.816 | 11,86% |

Segunda maior da história, atrás só da anterior. Concentração:

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| 100,0% | 1.123 / 1.123 | `runtime/pinker_json_contract/src/lib.rs` |
| 100,0% | 915 / 915 | `src/processo_estruturado_hospedado.rs` |
| 98,6% | 928 / 941 | `src/tooling.rs` |
| 25,3% | 1.770 / 6.987 | `runtime/pinker_rt/src/lib.rs` |
| 18,9% | 1.078 / 5.690 | `src/interpreter.rs` |
| 15,6% | 959 / 6.157 | `src/semantic.rs` |

mais ~9.500 linhas de testes rotulados `part_b`, `part_c`, `part_d`, `part_e1`,
`part_e2` e `issue497` — os mesmos rótulos sem endereço documental do achado
L-24.

---

## 2. O padrão que esta era estabeleceu, e que é bom

Esta janela criou os **crates de contrato** do workspace:

```
runtime/pinker_argv_contract      runtime/pinker_json_contract
runtime/pinker_memory_contract    runtime/pinker_sha256_contract
```

O desenho é o mesmo dos precedentes que `src/intrinsics/registry.rs` cita como
adultos — `falha_operacional`, `valor_json`, `sha256`, `saida_processo`: **uma
autoridade declara o contrato, e as duas execuções (interpretador hospedado e
runtime nativo) o consomem.**

É a resposta estrutural correta ao problema que esta série vem descrevendo desde
L-07 e L-12: quando um fato precisa valer em vários lugares, ele mora num lugar e
os outros consultam.

Registro isso com a mesma firmeza com que registrei L-20: **a Pinker de agosto de
2026 sabe fazer certo o que a Pinker de março não sabia.** Os crates de contrato,
o registry de intrínsecas e o runtime do Eixo B são três instâncias do mesmo
acerto, e nenhum deles gerou achado de dívida nesta auditoria.

O corolário prático é a razão de a série ser otimista: **os remédios para L-07,
L-12, L-10 e L-15 não precisam ser inventados.** Eles existem, funcionam, e estão
neste repositório. O que falta é aplicá-los aos pontos que ficaram para trás.

---

## 3. Nenhum achado novo de dívida

Esta é a primeira era da série que não produz achado próprio.

Os problemas que o código desta janela toca são todos herdados e já registrados:
`interpreter.rs` cresceu 1.078 linhas dentro do modelo de `RuntimeValue` de L-04;
`semantic.rs` cresceu 959 dentro do modelo de erro de L-05; os testes carregam os
rótulos de L-24.

Nada disso é falha desta era — é a definição de dívida: o custo aparece em quem
vem depois.

O único registro próprio é negativo e vale dizer: **procurei e não encontrei**
nesta janela nenhum caminho morto, nenhuma duplicação de autoridade, nenhuma
superfície congelada, nenhuma função pública órfã nova. Das vinte e uma funções
`pub` sem chamador do primeiro documento, esta era contribuiu com duas
(`tooling::binary_commit`, `tooling::recommended_action`) — ambas modernas, ambas
plausivelmente reservadas, ambas cobertas pela anotação de intenção que o gate de
alcançabilidade vai pedir.

---

## 4. Registro acumulado

Sem acréscimo. O registro segue com L-01 a L-24.

Anotação para a síntese final da série: esta era é a **prova de viabilidade** das
recomendações. Tudo o que os documentos anteriores pedem — autoridade única,
contrato declarado, execução dupla verificada por paridade — foi feito aqui, em
doze dias, sem reforma da pipeline e sem abrir exceção no roadmap.
