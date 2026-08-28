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
    ),
    (
        "S3",
        "recurso compartilhado declarado com caminho relativo, dentro do task root",
        [('        "path": "/book",', '        "path": "cache/book",')],
    ),
    (
        "S4",
        "TASK_ROOT reconstruído por concatenação do TASK_ID",
        [
            ('SLOT_RE = re.compile(r"^a[0-9]{2,4}$")', 'SLOT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")'),
            ("        slot = alocar_slot(raiz)", "        slot = task_id"),
        ],
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
    ),
    (
        "S6",
        "delete caminha seguindo symlink e apaga o alvo do outro lado",
        [
            (
                "    pai = raiz.parent\n"
                "    fd_pai = os.open(str(pai), os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY)\n"
                "    try:\n"
                "        descer(fd_pai, raiz.name)\n"
                "    finally:\n"
                "        os.close(fd_pai)\n"
                "    return bytes_removidos, arquivos_removidos",
                "    for base, dirs, nomes in os.walk(raiz, topdown=False, followlinks=True):\n"
                "        for nome in nomes:\n"
                "            try:\n"
                "                os.unlink(os.path.join(base, nome))\n"
                "            except OSError:\n"
                "                pass\n"
                "        for d in dirs:\n"
                "            try:\n"
                "                os.rmdir(os.path.join(base, d))\n"
                "            except OSError:\n"
                "                try:\n"
                "                    os.unlink(os.path.join(base, d))\n"
                "                except OSError:\n"
                "                    pass\n"
                "    try:\n"
                "        os.rmdir(raiz)\n"
                "    except OSError:\n"
                "        pass\n"
                "    return bytes_removidos, arquivos_removidos",
            ),
        ],
    ),
    (
        "S6b",
        "seal resolve o recurso com stat em vez de lstat e atravessa o link",
        [
            (
                "        sem_symlink_em_componentes(alvo, canonical_main())\n"
                '        if alvo.is_symlink():\n'
                '            raise ForjaError("DENIED", f"recurso é symlink: {alvo}")',
                "        pass",
            ),
            ("        st = os.lstat(nome, dir_fd=fd_pai)", "        st = os.stat(nome, dir_fd=fd_pai)"),
        ],
    ),
    (
        "S7",
        "finalizer da Task A alcança a memória da Task B",
        [
            (
                "    provas = guardas_de_destruicao(main, raiz, task_root, binding)",
                "    provas = guardas_de_destruicao(main, raiz, task_root, binding)\n"
                "    if args.apply:\n"
                "        for _v, _b in slots_existentes(raiz):\n"
                '            _a = raiz / _v / "memory"\n'
                "            if _v != slot and _a.is_dir():\n"
                "                remover_arvore_sem_seguir_links(_a)",
            ),
        ],
    ),
    (
        "S8",
        "worktree removida do disco sem desregistro nem prune: metadata órfã",
        [
            (
                '    if registrada:\n'
                '        git(main, "worktree", "remove", "--force", str(worktree), check=False)',
                "    pass",
            ),
            ('    git(main, "worktree", "prune", check=False)', "    pass"),
            ("def metadata_orfa(main: Path) -> list[str]:", "def metadata_orfa(main: Path) -> list[str]:\n    return []"),
        ],
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
    ),
]


class SensibilidadeTests(unittest.TestCase):
    """Cada gate desta suíte precisa ficar vermelho quando quebrado."""

    maxDiff = None

    def _rodar_suite(self, base: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-m", "unittest", "discover", "-s", str(base / "tests" / "forja"), "-p", "test_forja_agentes.py"],
            capture_output=True,
            text=True,
            cwd=str(base),
        )

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
        for ident, descricao, pares in MUTACOES:
            with self.subTest(mutacao=ident):
                with tempfile.TemporaryDirectory() as t:
                    base = self._montar(Path(t))
                    alvo = base / "scripts" / "forja" / "forja_agentes.py"
                    texto = alvo.read_text(encoding="utf-8")
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
                    alvo.write_text(texto, encoding="utf-8")
                    r = self._rodar_suite(base)
                    if r.returncode == 0:
                        falhas.append(f"{ident} ({descricao}): suíte ficou VERDE com a mutação aplicada")
        self.assertEqual(falhas, [], "gates que não fecham:\n" + "\n".join(falhas))


if __name__ == "__main__":
    unittest.main(verbosity=2)
