#!/usr/bin/env python3
"""Sensibilidade dos gates da autoridade de layout.

Um gate que nunca ficou vermelho não prova nada. Cada caso aqui aplica uma
mutação reversível que quebra deliberadamente uma propriedade, roda a suíte
contra a versão mutada e **exige** falha.

A mutação nunca toca a árvore de trabalho: a fonte é copiada para um diretório
temporário e a suíte roda contra a cópia. Isso permite rodar no CI sem risco.

Duas armadilhas que esta suíte fecha explicitamente:

- uma mutação que não aplica parece um gate passando — por isso cada caso prova
  que o texto mudou antes de julgar o resultado;
- uma mutação que quebra o import parece um gate fechando — por isso o modo
  esperado distingue falha de teste de erro de coleta quando relevante.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTE = RAIZ / "scripts" / "forja" / "forja_agentes.py"
SUITE = Path(__file__).resolve().parent / "test_forja_agentes.py"

# (id, descrição, [(trecho_original, trecho_mutado), ...])
MUTACOES: list[tuple[str, str, list[tuple[str, str]]]] = [
    (
        "S1",
        "contenção por prefixo textual deixa `..` escapar do task root",
        [
            (
                '    c = os.path.normpath(os.path.abspath(caminho))\n'
                '    r = os.path.normpath(os.path.abspath(raiz))\n'
                '    return c == r or c.startswith(r + os.sep)',
                '    return str(caminho).startswith(str(raiz) + os.sep)',
            ),
        ],
        "test_prefixo_textual_nao_e_contencao",
    ),
    (
        "S2",
        "finalizer ignora um recurso que o provisionador cria",
        [
            (
                "        if recurso.classe != Classe.EPHEMERAL:",
                '        if recurso.classe != Classe.EPHEMERAL or recurso.nome == "target":',
            ),
        ],
        "test_seal_preserva_worktree_memoria_e_estado",
    ),
    (
        "S3",
        "recurso compartilhado declarado com caminho relativo, dentro do task root",
        [
            (
                "for _c in COMPARTILHADOS:\n"
                '    if not os.path.isabs(_c["path"]):\n'
                '        raise RuntimeError(f"contrato inválido: recurso compartilhado sem caminho absoluto: {_c}")\n'
                "del _c",
                "pass",
            ),
            ('        "path": "/book",', '        "path": "cache/book",'),
        ],
        "test_recursos_compartilhados_ficam_fora_do_task_root",
    ),
    (
        "S4",
        "TASK_ROOT reconstruído por concatenação do TASK_ID",
        [
            ('SLOT_RE = re.compile(r"^a[0-9]{2,4}$")', 'SLOT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")'),
            ("        slot = reivindicar_slot(raiz)  # já criou o diretório, atomicamente",
             "        slot = task_id\n        os.makedirs(raiz / slot, exist_ok=True)"),
        ],
        "test_slot_nao_e_derivado_do_task_id",
    ),
    (
        "S5",
        "cleanup aceita o checkout canônico como alvo de destruição",
        [
            (
                '    if real_root.parent != real_raiz:\n'
                '        raise ForjaError("DENIED", f"task root não está contido em {real_raiz}")',
                "    if False:\n        pass",
            ),
            (
                '    if real_root == real_main or real_root == real_raiz:\n'
                '        raise ForjaError("DENIED", "task root coincide com o checkout canônico ou com agentes/")',
                "    if False:\n        pass",
            ),
            (
                '    if contido_em(real_main, real_root):\n'
                '        raise ForjaError("DENIED", "checkout canônico está contido no task root")',
                "    if False:\n        pass",
            ),
            (
                '    if not SLOT_RE.fullmatch(real_root.name):\n'
                '        raise ForjaError("DENIED", f"nome de slot inválido para destruição: {real_root.name}")',
                "    if False:\n        pass",
            ),
        ],
        "test_nunca_apaga_o_checkout_candidato_recusa_por_contencao",
    ),
    (
        "S6",
        "delete resolve o nome com stat e atravessa o symlink",
        [
            # Mutante mínimo: trocar lstat por stat faz `descer` seguir o link e
            # apagar o alvo do outro lado. Uma linha, estável a refatoração do
            # corpo — a versão anterior mutava o bloco inteiro e quebrava a cada
            # mudança vizinha, custando um ciclo de diagnóstico por rodada.
            # A proteção real é o O_NOFOLLOW, não o lstat: com `stat` o Git
            # chega a achar que o link é diretório, mas o open recusa com
            # "Not a directory" e o mutante morre por erro em vez de escapar —
            # vermelho que não exercita a defesa. Remover o O_NOFOLLOW é o
            # mutante que produz o escape de verdade.
            (
                "            fd = os.open(nome, os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY, dir_fd=fd_pai)",
                "            fd = os.open(nome, os.O_RDONLY | os.O_DIRECTORY, dir_fd=fd_pai)",
            ),
            ("        st = os.lstat(nome, dir_fd=fd_pai)", "        st = os.stat(nome, dir_fd=fd_pai)"),
        ],
        "test_nao_segue_symlink_para_fora_ao_apagar",
    ),
    (
        "S6b",
        "seal deixa de recusar recurso que é symlink",
        [
            # Distinto do S6 de propósito: aqui o delete continua sem seguir
            # link (lstat intacto), então o escape não acontece. O que se perde
            # é a RECUSA explícita — o selo passaria a tratar um recurso
            # symlink como recurso comum em vez de bloquear.
            (
                "        sem_symlink_em_componentes(alvo, canonical_main())\n        if stat.S_ISLNK(alvo.lstat().st_mode):\n            raise ForjaError(\"DENIED\", f\"recurso \u00e9 symlink: {alvo}\")",
                "        pass",
            ),
        ],
        "test_recusa_componente_symlink_no_caminho",
    ),
    (
        "S7",
        "finalizer da Task A alcança a memória da Task B",
        [
            (
                "    provas, identidade = guardas_de_destruicao(main, raiz, task_root, binding)",
                "    provas, identidade = guardas_de_destruicao(main, raiz, task_root, binding)\n"
                "    if args.apply:\n"
                "        for _v, _b in slots_existentes(raiz):\n"
                '            _a = raiz / _v / "memory"\n'
                "            if _v != slot and _a.is_dir():\n"
                "                remover_arvore_sem_seguir_links(_a)",
            ),
        ],
        "test_retirar_a_nao_toca_b",
    ),
    (
        "S8",
        "worktree removida do disco sem desregistro nem prune: metadata órfã",
        [
            ("    if registrada:", "    if False:"),
            ("    r = git(main, \"worktree\", \"prune\", check=False)", "    r = type(\"R\",(),{\"returncode\":0,\"stderr\":\"\"})()"),
            (
                "    if orfas:\n"
                "        raise ForjaError(\n"
                '            "FAILED",\n'
                '            f"metadata Git órfã após a retirada (root_removed={ausente}): {orfas}",\n'
                "        )",
                "    pass",
            ),
            (
                "def metadata_orfa(main: Path) -> list[str]:",
                "def metadata_orfa(main: Path) -> list[str]:\n    return []",
            ),
        ],
        "test_retire_nao_deixa_metadata_orfa",
    ),
    (
        "S10",
        "retire ignora o estado de lifecycle e destrói Task em revisão",
        [
            (
                '    estado = (binding or {}).get("state")\n'
                "    if estado not in ESTADOS_DESTRUTIVEIS:\n"
                '        raise ForjaError("DENIED", f"estado {estado!r} não autoriza destruição; exigido um de {ESTADOS_DESTRUTIVEIS}")',
                '    estado = (binding or {}).get("state")',
            ),
        ],
        "test_retire_exige_estado_elegivel",
    ),
    (
        "S17",
        "comando mutante aceita --task-id de outra Task: A apaga B pela interface",
        [
            (
                "def resolver_task_propria(arg: str | None) -> str:",
                "def resolver_task_propria(arg: str | None) -> str:\n"
                "    if arg:\n"
                "        return validar_task(arg)",
            ),
        ],
        "test_retire_recusa_task_de_outrem_via_task_id",
    ),
    (
        "S18",
        "state permite qualquer transição, inclusive ACTIVE -> RETIREABLE",
        [
            (
                "    if args.set != anterior and args.set not in TRANSICOES.get(anterior, ()):\n"
                "        raise ForjaError(\n"
                '            "DENIED",\n'
                '            f"transição não autorizada: {anterior} -> {args.set}; de {anterior} só é permitido {TRANSICOES.get(anterior, ())}",\n'
                "        )",
                "    pass",
            ),
        ],
        "test_active_nao_salta_direto_para_retireable",
    ),
    (
        "S19",
        "retire trata falha do Git como sucesso terminal",
        [
            # Mutantes mínimos de uma linha: desligam as duas guardas sem
            # depender do texto do corpo, que muda a cada correção vizinha.
            ("        if r.returncode != 0:", "        if False:"),
            ("        if worktree_registrada(main, worktree):", "        if False:"),
        ],
        "test_retire_falha_quando_o_desregistro_falha",
    ),
    (
        "S20",
        "reivindicação de slot volta a ser scan-then-use: planeja sem marcar",
        [
            (
                "        try:\n"
                "            os.mkdir(alvo, MODO_DIR)\n"
                "        except FileExistsError:\n"
                "            continue  # perdemos a corrida para outro provisionamento: siga adiante\n"
                "        try:\n"
                "            os.chmod(alvo, MODO_DIR)\n"
                "            gid = gid_agentes()\n"
                "            if gid is not None and alvo.lstat().st_gid != gid:\n"
                "                os.chown(alvo, -1, gid)\n"
                "        except PermissionError:\n"
                "            pass\n"
                "        return candidato",
                "        if alvo.exists():\n"
                "            continue\n"
                "        return candidato",
            ),
        ],
        "test_reivindicar_deixa_marca_para_o_proximo_chamador",
    ),
    (
        "S21",
        "vínculo do slot deixa de ser conferido: mutação em root de outro agente",
        [
            (
                '    dono = binding.get("agent")\n'
                "    if dono and dono != agente_corrente():\n"
                '        raise ForjaError("DENIED", f"task root pertence ao agente {dono!r}, não a {agente_corrente()!r}")',
                "    pass",
            ),
        ],
        "test_mutacao_recusa_root_de_outro_agente",
    ),
    (
        "S16",
        "provision cria o recurso sem provar contenção",
        [
            (
                '        alvo = exigir_contido(task_root / recurso.subpath, task_root, f"recurso {recurso.nome}")\n'
                '        if recurso.nome == "worktree":',
                '        alvo = task_root / recurso.subpath\n        if recurso.nome == "worktree":',
            ),
            (
                'for _r in RECURSOS:\n'
                '    if os.path.isabs(_r.subpath) or os.path.normpath(_r.subpath).startswith(".."):\n'
                '        raise RuntimeError(f"contrato inválido: recurso {_r.nome} escapa do task root: {_r.subpath}")\n'
                '    if os.path.normpath(_r.subpath) != _r.subpath:\n'
                '        raise RuntimeError(f"contrato inválido: subpath não normalizado: {_r.subpath}")\n'
                'del _r',
                'pass',
            ),
            ('Recurso("logs", "logs",', 'Recurso("logs", "../logs",'),
        ],
        "test_provision_nao_cria_nada_fora_do_root_nem_ao_falhar",
    ),
    (
        "S22",
        "caminho destrutivo volta ao predicado booleano de existência",
        [
            (
                '    if estado_do_caminho(task_root, "task root") == PRESENTE:',
                "    if task_root.exists():",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S23",
        "selo remove o recurso sem fixar identidade",
        [
            (
                "            remover_arvore_sem_seguir_links(alvo, identidade_alvo)",
                "            remover_arvore_sem_seguir_links(alvo)",
            ),
        ],
        "test_seal_recusa_quando_o_recurso_e_trocado_apos_a_guarda",
    ),
    (
        "S24",
        "predicado booleano volta por helper externo ao caminho destrutivo",
        [
            (
                '    if estado_do_caminho(task_root, "task root") == AUSENTE:',
                "    if not _helper_existe(task_root):",
            ),
            (
                "def exigir_raizes() -> tuple[Path, Path]:",
                "def _helper_existe(p):\n    return p.exists()\n\n\ndef exigir_raizes() -> tuple[Path, Path]:",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S25",
        "predicado booleano volta por getattr de nome montado em runtime",
        [
            (
                '    if estado_do_caminho(task_root, "task root") == AUSENTE:',
                '    if not getattr(task_root, "exi" + "sts")():',
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S26",
        "consumidor engole a recusa tri-state com append em lista descartada",
        [
            (
                "        except ForjaError as erro:\n"
                '            achados.append(f"{wt} (inspeção falhou: {erro.mensagem[:60]})")',
                "        except ForjaError as erro:\n"
                "            _descarte = []\n"
                "            _descarte.append(str(erro))\n"
                "            continue",
            ),
        ],
        "test_inspecao_que_falha_aparece_no_resultado",
    ),
    (
        "S27",
        "prova de symlink volta ao fail-open na própria função de prova",
        [
            (
                '    if e_symlink(base, "base"):',
                "    if base.is_symlink():",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S28",
        "predicado proibido entra na decisão por alias de módulo",
        [
            (
                'PRESENTE, AUSENTE = "PRESENTE", "AUSENTE"',
                'PRESENTE, AUSENTE = "PRESENTE", "AUSENTE"\nALIAS_EXISTE = Path.exists',
            ),
            (
                '    if estado_do_caminho(task_root, "task root") == PRESENTE:',
                "    if ALIAS_EXISTE(task_root):",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S29",
        "detector reporta resíduo sem consultar o registro da worktree",
        [
            (
                "        try:\n"
                "            if not worktree_registrada(main, wt):\n"
                "                achados.append(str(wt))\n"
                "        except ForjaError as erro:\n"
                '            achados.append(f"{wt} (inspeção falhou: {erro.mensagem[:60]})")',
                "        achados.append(str(wt))",
            ),
        ],
        "test_inspecao_que_falha_aparece_no_resultado",
    ),
    (
        "S30",
        "verify volta a aceitar slot inobservável como não-symlink",
        [
            (
                "        try:\n"
                '            if e_symlink(task_root, f"slot {nome}"):\n'
                '                problemas.append(f"slot é symlink: {nome}")\n'
                "        except ForjaError as erro:\n"
                '            problemas.append(f"slot {nome} inobservável: {erro.mensagem}")',
                "        if task_root.is_symlink():\n"
                '            problemas.append(f"slot é symlink: {nome}")',
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S31",
        "predicado proibido entra por import direto, sem atributo nem atribuição",
        [
            (
                "import stat",
                "import stat\nfrom os.path import isdir as _existe_dir",
            ),
            (
                '    if estado_do_caminho(task_root, "task root") == AUSENTE:',
                "    if not _existe_dir(task_root):",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S32",
        "ocupante inválido volta a ser tratado como worktree presente",
        [
            (
                "    if args.branch and _worktree_a_criar(worktree, raiz):",
                '    if args.branch and estado_do_caminho(worktree, "worktree") == AUSENTE:',
            ),
        ],
        "test_worktree_symlink_pendente_e_recusada_e_nao_provisionada",
    ),
    (
        "S33",
        "medição volta a engolir arquivo ilegível sem deixar rastro",
        [
            (
                "                ilegiveis[0] += 1\n                continue",
                "                continue",
            ),
        ],
        "test_arquivo_ilegivel_e_contado_como_ilegivel_e_nao_sumido",
    ),
    (
        "S34",
        "predicado proibido entra por cadeia de alias de dois passos",
        [
            (
                'PRESENTE, AUSENTE = "PRESENTE", "AUSENTE"',
                'PRESENTE, AUSENTE = "PRESENTE", "AUSENTE"\n_A1 = Path.exists\n_A2 = _A1',
            ),
            (
                '    if estado_do_caminho(task_root, "task root") == PRESENTE:',
                "    if _A2(task_root):",
            ),
        ],
        "test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia",
    ),
    (
        "S35",
        "callback de os.walk volta a descartar falha de travessia",
        [
            ("onerror=registrar_falha_ao_descer", "onerror=lambda _e: None"),
        ],
        "test_subdiretorio_ilegivel_nao_some_da_medicao",
    ),
    (
        "S36",
        "selo volta a descartar o que não conseguiu medir",
        [
            (
                '                "unreadable_before": ilegiveis,\n',
                "",
            ),
        ],
        "test_selo_publica_o_que_nao_conseguiu_medir",
    ),
    (
        "S37",
        "provision deixa de validar o tipo do task root",
        [
            (
                '    if not e_diretorio(task_root, "task root") or e_symlink(task_root, "task root"):',
                "    if False:",
            ),
        ],
        "test_task_root_inobservavel_impede_provisionar",
    ),
    (
        "S38",
        "verify deixa de checar o tipo do slot",
        [
            (
                "        try:\n"
                '            if e_symlink(task_root, f"slot {nome}"):\n'
                '                problemas.append(f"slot é symlink: {nome}")\n'
                "        except ForjaError as erro:\n"
                '            problemas.append(f"slot {nome} inobservável: {erro.mensagem}")',
                "        pass",
            ),
        ],
        "test_slot_inobservavel_reprova_a_verificacao",
    ),
]


class SensibilidadeTests(unittest.TestCase):
    """Cada gate desta suíte precisa ficar vermelho quando quebrado."""

    maxDiff = None

    def _rodar_suite(self, base: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-m", "unittest", "discover", "-s", str(base / "tests" / "forja"), "-p", "test_forja_agentes.py", "-v"],
            capture_output=True,
            text=True,
            cwd=str(base),
        )

    @staticmethod
    def _quebrou(saida: str, teste: str) -> tuple[bool, str]:
        """O teste nomeado falhou, e devolve também POR QUE falhou.

        Exigir só `FAIL:`/`ERROR:` do nome certo ainda deixa passar o mutante
        que derruba o alvo por efeito colateral — um `ENOTDIR` levantado antes
        do oráculo, por exemplo. O motivo é extraído para que o caso possa
        exigir causalidade, e não apenas coincidência de nome.
        """
        linhas = saida.splitlines()
        for i, linha in enumerate(linhas):
            if linha.startswith(("FAIL: " + teste, "ERROR: " + teste)):
                for detalhe in linhas[i : i + 40]:
                    d = detalhe.strip()
                    if d.startswith(("AssertionError", "ForjaError", "OSError", "NotADirectoryError",
                                     "FileNotFoundError", "PermissionError", "TypeError", "ValueError")):
                        return True, d[:200]
                return True, "(motivo não extraído)"
        return False, ""

    def _montar(self, tmp: Path) -> Path:
        base = tmp / "arvore"
        (base / "scripts" / "forja").mkdir(parents=True)
        (base / "tests" / "forja").mkdir(parents=True)
        shutil.copy2(FONTE, base / "scripts" / "forja" / "forja_agentes.py")
        shutil.copy2(SUITE, base / "tests" / "forja" / "test_forja_agentes.py")
        return base

    def test_suite_intacta_fica_verde(self) -> None:
        with tempfile.TemporaryDirectory() as t:
            base = self._montar(Path(t))
            r = self._rodar_suite(base)
            self.assertEqual(r.returncode, 0, f"linha de base não está verde:\n{r.stderr[-4000:]}")

    def test_cada_mutacao_deixa_a_suite_vermelha(self) -> None:
        falhas: list[str] = []
        observados: dict[str, str] = {}
        for ident, descricao, pares, alvo in MUTACOES:
            with self.subTest(mutacao=ident):
                with tempfile.TemporaryDirectory() as t:
                    base = self._montar(Path(t))
                    arquivo = base / "scripts" / "forja" / "forja_agentes.py"
                    texto = arquivo.read_text(encoding="utf-8")
                    original = texto
                    for antigo, novo in pares:
                        self.assertIn(
                            antigo,
                            texto,
                            f"{ident}: trecho a mutar não existe mais na fonte — "
                            "atualize a mutação, não a conclusão",
                        )
                        texto = texto.replace(antigo, novo, 1)
                    self.assertNotEqual(
                        texto,
                        original,
                        f"{ident}: a mutação não alterou nada; o resultado seria um falso verde",
                    )
                    arquivo.write_text(texto, encoding="utf-8")
                    r = self._rodar_suite(base)
                    saida = r.stdout + r.stderr
                    quebrou, motivo = self._quebrou(saida, alvo)
                    if r.returncode == 0:
                        falhas.append(f"{ident} ({descricao}): suíte ficou VERDE com a mutação aplicada")
                    elif not quebrou:
                        falhas.append(
                            f"{ident} ({descricao}): suíte ficou vermelha, mas NÃO por {alvo} — "
                            "vermelho pelo motivo errado não prova o gate"
                        )
                    else:
                        # causalidade: o alvo tem de cair pela propriedade, e não
                        # por uma exceção do sistema levantada antes do oráculo
                        # Regra invertida: só AssertionError prova que o oráculo
                        # do teste disparou. Enumerar exceções "acidentais" era
                        # uma lista que sempre esqueceria a próxima — foi o que
                        # aconteceu com FileNotFoundError no S7.
                        if not motivo.startswith("AssertionError"):
                            falhas.append(
                                f"{ident} ({descricao}): {alvo} caiu por efeito colateral "
                                f"({motivo[:90]}) e não pela propriedade sob teste"
                            )
                        else:
                            observados[ident] = motivo
        if falhas:
            print("\nmotivos observados por mutante:")
            for k, v in sorted(observados.items()):
                print(f"  {k}: {v[:110]}")
        self.assertEqual(falhas, [], "gates que não fecham:\n" + "\n".join(falhas))


if __name__ == "__main__":
    unittest.main(verbosity=2)
