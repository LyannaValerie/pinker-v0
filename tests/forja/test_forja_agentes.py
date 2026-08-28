#!/usr/bin/env python3
"""Suíte da autoridade de layout `agentes/`.

Roda sem root, sem host da Forja e sem rede: todo o layout é montado num
diretório temporário sob modo de teste explícito. Os testes que importam são os
de recusa — um cleanup que só é testado no caminho feliz não prova nada.
"""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTE = RAIZ / "scripts" / "forja" / "forja_agentes.py"


def carregar_modulo():
    spec = importlib.util.spec_from_file_location("forja_agentes", FONTE)
    assert spec and spec.loader
    modulo = importlib.util.module_from_spec(spec)
    # Registrar antes de executar: `dataclasses` resolve anotações pelo módulo
    # em `sys.modules`, e um módulo carregado fora dele quebra em Python 3.14.
    sys.modules[spec.name] = modulo
    spec.loader.exec_module(modulo)
    return modulo


fa = carregar_modulo()


def git(caminho: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(caminho), *args],
        capture_output=True,
        text=True,
        check=True,
        env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"},
    )


class Base(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="forja-agentes-t"))
        self.main = self.tmp / "pinker-v0"
        self.main.mkdir()
        git(self.main, "init", "-q", "-b", "main")
        git(self.main, "config", "user.email", "t@t")
        git(self.main, "config", "user.name", "t")
        (self.main / "README.md").write_text("x\n", encoding="utf-8")
        agentes = self.main / "agentes"
        agentes.mkdir()
        (agentes / ".gitignore").write_text("*\n!.gitignore\n!README.md\n", encoding="utf-8")
        (agentes / "README.md").write_text("politica\n", encoding="utf-8")
        git(self.main, "add", "-A")
        git(self.main, "commit", "-qm", "base")
        self.env = {
            "FORJA_AGENTES_TEST_MODE": "1",
            "FORJA_AGENTES_TEST_MAIN": str(self.main),
            "FORJA_AGENTES_TEST_TASK": "issue-536-exemplo-de-identidade-longa",
            "FORJA_AGENTES_TEST_AGENT": "agentetest",
        }
        self._antigo = {k: os.environ.get(k) for k in self.env}
        os.environ.update(self.env)

    def tearDown(self) -> None:
        for k, v in self._antigo.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        shutil.rmtree(self.tmp, ignore_errors=True)

    def rodar(self, *argv: str) -> tuple[int, dict]:
        import io
        from contextlib import redirect_stdout, redirect_stderr

        saida, erro = io.StringIO(), io.StringIO()
        with redirect_stdout(saida), redirect_stderr(erro):
            codigo = fa.main(list(argv))
        bruto = saida.getvalue().strip() or erro.getvalue().strip()
        try:
            return codigo, json.loads(bruto)
        except json.JSONDecodeError:
            return codigo, {"raw": bruto}

    def provisionar(self, branch: str = "b1") -> dict:
        codigo, dados = self.rodar("provision", "--branch", branch, "--base", "main")
        self.assertEqual(codigo, 0, dados)
        return dados


class ContratoTests(Base):
    def test_provisioner_set_igual_finalizer_set(self) -> None:
        codigo, dados = self.rodar("contract")
        self.assertEqual(codigo, 0)
        self.assertEqual(sorted(dados["provisioner_set"]), sorted(dados["finalizer_set"]))

    def test_toda_classe_de_recurso_e_conhecida(self) -> None:
        for recurso in fa.RECURSOS:
            self.assertIn(recurso.classe, (fa.Classe.EPHEMERAL, fa.Classe.DURABLE))

    def test_recursos_compartilhados_ficam_fora_do_task_root(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        for compartilhado in fa.COMPARTILHADOS:
            # absoluto primeiro: um caminho relativo resolveria contra o cwd e
            # passaria pela contenção por acidente, não por estar fora
            self.assertTrue(
                os.path.isabs(compartilhado["path"]),
                f"recurso compartilhado com caminho relativo: {compartilhado}",
            )
            self.assertFalse(
                fa.contido_em(Path(compartilhado["path"]), raiz),
                f"recurso compartilhado escondido dentro do task root: {compartilhado}",
            )


class ProvisionamentoTests(Base):
    def test_provision_cria_todo_o_conjunto(self) -> None:
        dados = self.provisionar()
        self.assertEqual(dados["status"], "PROVISIONED")
        for recurso in dados["resources"]:
            self.assertTrue(recurso["present"], recurso)

    def test_todo_recurso_esta_contido_no_task_root(self) -> None:
        dados = self.provisionar()
        raiz = dados["task_root"]
        for recurso in dados["resources"]:
            # contenção canônica: prefixo textual deixaria `raiz/../x` passar
            self.assertTrue(fa.contido_em(Path(recurso["path"]), Path(raiz)), recurso)
            self.assertEqual(os.path.normpath(recurso["path"]), recurso["path"], recurso)

    def test_prefixo_textual_nao_e_contencao(self) -> None:
        raiz = Path("/pinker/repo/pinker-v0/agentes/a01")
        fuga = raiz / ".." / "a02" / "target"
        self.assertTrue(str(fuga).startswith(str(raiz) + os.sep), "premissa do teste")
        self.assertFalse(fa.contido_em(fuga, raiz), "`..` escapou pela comparação de prefixo")
        self.assertTrue(fa.contido_em(raiz / "target", raiz))

    def test_contrato_recusa_subpath_com_traversal(self) -> None:
        for mau in ("../logs", "/absoluto", "a/../../b"):
            self.assertTrue(
                os.path.isabs(mau)
                or os.path.normpath(mau).startswith("..")
                or os.path.normpath(mau) != mau,
                mau,
            )
        with self.assertRaises(fa.ForjaError):
            fa.exigir_contido(Path("/x/a01/../a02"), Path("/x/a01"), "teste")

    def test_slot_nao_e_derivado_do_task_id(self) -> None:
        dados = self.provisionar()
        self.assertNotEqual(dados["slot"], dados["task_id"])
        self.assertNotIn(dados["task_id"], dados["task_root"])

    def test_task_root_nao_e_reconstruivel_por_concatenacao(self) -> None:
        dados = self.provisionar()
        concatenado = os.path.join(dados["agentes_root"], dados["task_id"])
        self.assertNotEqual(concatenado, dados["task_root"])
        self.assertFalse(os.path.exists(concatenado))

    def test_vinculo_e_observado_e_nao_presumido(self) -> None:
        dados = self.provisionar()
        binding = json.loads((Path(dados["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding["task_id"], dados["task_id"])
        self.assertEqual(binding["slot"], dados["slot"])
        codigo, observado = self.rodar("observe")
        self.assertEqual(codigo, 0)
        self.assertEqual(observado["task_root"], dados["task_root"])

    def test_provision_e_idempotente(self) -> None:
        a = self.provisionar()
        codigo, b = self.rodar("provision")
        self.assertEqual(codigo, 0)
        self.assertEqual(a["slot"], b["slot"])
        self.assertFalse(b["slot_created"])

    def test_worktree_e_registrada_pelo_git(self) -> None:
        dados = self.provisionar()
        saida = git(self.main, "worktree", "list", "--porcelain").stdout
        self.assertIn(str(Path(dados["task_root"]) / "worktree"), saida)

    def test_checkout_canonico_permanece_limpo(self) -> None:
        self.provisionar()
        saida = git(self.main, "status", "--porcelain").stdout.strip()
        self.assertEqual(saida, "", f"task root sujou o checkout canônico: {saida}")

    def test_env_exporta_apenas_paths_do_task_root(self) -> None:
        dados = self.provisionar()
        import io
        from contextlib import redirect_stdout

        saida = io.StringIO()
        with redirect_stdout(saida):
            fa.main(["env"])
        linhas = [x for x in saida.getvalue().splitlines() if x.startswith("export ")]
        self.assertTrue(linhas)
        for linha in linhas:
            valor = linha.split("=", 1)[1]
            if valor in (dados["task_id"], dados["canonical_main"], dados["task_root"]):
                continue
            self.assertTrue(valor.startswith(dados["task_root"] + os.sep), linha)


class ConcorrenciaTests(Base):
    def test_dois_task_roots_sao_disjuntos(self) -> None:
        a = self.provisionar("b1")
        os.environ["FORJA_AGENTES_TEST_TASK"] = "outra-task-b-com-identidade-longa"
        codigo, b = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertEqual(codigo, 0, b)
        self.assertNotEqual(a["task_root"], b["task_root"])
        self.assertFalse(a["task_root"].startswith(b["task_root"] + os.sep))
        self.assertFalse(b["task_root"].startswith(a["task_root"] + os.sep))

    def test_retirar_a_nao_toca_b(self) -> None:
        a = self.provisionar("b1")
        (Path(a["task_root"]) / "target" / "peso.bin").write_bytes(b"a" * 512)
        os.environ["FORJA_AGENTES_TEST_TASK"] = "outra-task-b-com-identidade-longa"
        codigo, b = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertEqual(codigo, 0)
        marca = Path(b["task_root"]) / "memory" / "events.jsonl"
        marca.write_text('{"kind":"marca"}\n', encoding="utf-8")
        antes = marca.read_bytes()

        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        self.rodar("state", "--set", "RETIREABLE")
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertTrue(resultado["task_root_absent"])

        self.assertTrue(Path(b["task_root"]).is_dir(), "retire(A) destruiu B")
        self.assertEqual(marca.read_bytes(), antes, "retire(A) mutou a memória de B")
        saida = git(self.main, "worktree", "list", "--porcelain").stdout
        self.assertIn(str(Path(b["task_root"]) / "worktree"), saida)

    def test_selar_a_nao_toca_b(self) -> None:
        a = self.provisionar("b1")
        os.environ["FORJA_AGENTES_TEST_TASK"] = "outra-task-b-com-identidade-longa"
        codigo, b = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertEqual(codigo, 0)
        peso_b = Path(b["task_root"]) / "target" / "peso.bin"
        peso_b.write_bytes(b"b" * 4096)

        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        peso_a = Path(a["task_root"]) / "target" / "peso.bin"
        peso_a.write_bytes(b"a" * 4096)
        codigo, resultado = self.rodar("seal", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertFalse(peso_a.exists())
        self.assertTrue(peso_b.exists(), "seal(A) consumiu o target de B")
        self.assertEqual(peso_b.stat().st_size, 4096)


class SeloTests(Base):
    def test_seal_preserva_worktree_memoria_e_estado(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        (raiz / "memory" / "events.jsonl").write_text('{"kind":"x"}\n', encoding="utf-8")
        (raiz / "state" / "checkpoint.json").write_text("{}\n", encoding="utf-8")
        (raiz / "artifacts" / "prova.txt").write_text("p\n", encoding="utf-8")
        (raiz / "target" / "peso.bin").write_bytes(b"z" * 8192)
        codigo, resultado = self.rodar("seal", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertGreaterEqual(resultado["reclaimed_bytes"], 8192)
        self.assertTrue((raiz / "memory" / "events.jsonl").exists())
        self.assertTrue((raiz / "state" / "checkpoint.json").exists())
        self.assertTrue((raiz / "artifacts" / "prova.txt").exists())
        self.assertTrue((raiz / "worktree" / "README.md").exists())
        self.assertFalse((raiz / "target" / "peso.bin").exists())
        self.assertTrue((raiz / "target").is_dir())

    def test_seal_sem_apply_nao_remove_nada(self) -> None:
        dados = self.provisionar()
        peso = Path(dados["task_root"]) / "target" / "peso.bin"
        peso.write_bytes(b"z" * 1024)
        codigo, resultado = self.rodar("seal")
        self.assertEqual(codigo, 0)
        self.assertEqual(resultado["status"], "SEAL_PLAN")
        self.assertTrue(peso.exists())

    def test_seal_deixa_a_task_recuperavel(self) -> None:
        dados = self.provisionar()
        self.rodar("seal", "--apply")
        codigo, observado = self.rodar("observe")
        self.assertEqual(codigo, 0)
        self.assertEqual(observado["task_root"], dados["task_root"])
        self.assertEqual(observado["state"], "SEALED")
        saida = git(Path(dados["task_root"]) / "worktree", "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
        self.assertEqual(saida, "b1")


class RetiradaTests(Base):
    def test_retire_exige_estado_elegivel(self) -> None:
        self.provisionar()
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO)
        self.assertEqual(resultado["status"], "DENIED")
        self.assertIn("ACTIVE", resultado["error"])

    def test_retire_recusa_estado_de_revisao(self) -> None:
        self.provisionar()
        for estado in ("REVIEW", "FIX_REQUIRED"):
            self.rodar("state", "--set", estado)
            codigo, _ = self.rodar("retire", "--apply")
            self.assertEqual(codigo, fa.EXIT_RECUSADO, estado)

    def test_retire_remove_registro_e_diretorio_da_worktree(self) -> None:
        dados = self.provisionar()
        self.rodar("state", "--set", "RETIREABLE")
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertFalse(Path(dados["task_root"]).exists())
        self.assertEqual(resultado["stale_git_worktree_metadata"], [])
        saida = git(self.main, "worktree", "list", "--porcelain").stdout
        self.assertNotIn(str(Path(dados["task_root"]) / "worktree"), saida)

    def test_retire_nao_deixa_metadata_orfa(self) -> None:
        dados = self.provisionar()
        self.rodar("state", "--set", "RETIREABLE")
        self.rodar("retire", "--apply")
        base = self.main / ".git" / "worktrees"
        restante = sorted(p.name for p in base.iterdir()) if base.is_dir() else []
        self.assertEqual(restante, [])

    def test_retire_reporta_a_branch_que_sobrevive(self) -> None:
        dados = self.provisionar("b1")
        raiz = Path(dados["task_root"])
        (raiz / "worktree" / "novo.txt").write_text("trabalho\n", encoding="utf-8")
        git(raiz / "worktree", "add", "-A")
        git(raiz / "worktree", "commit", "-qm", "trabalho da task")
        self.rodar("state", "--set", "RETIREABLE")
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, resultado)
        restante = resultado["branch_left_behind"]
        self.assertIsNotNone(restante, "retire apagou o root sem dizer que a branch sobrevive")
        self.assertEqual(restante["name"], "b1")
        self.assertEqual(restante["disposition"], "PRESERVED_NOT_DELETED_BY_RETIRE")
        # a branch e o commit continuam existindo: retirada destroi recurso, nao historico
        saida = git(self.main, "rev-parse", "--verify", "refs/heads/b1").stdout.strip()
        self.assertTrue(saida)

    def test_slot_e_reutilizavel_apos_retirada(self) -> None:
        a = self.provisionar()
        self.rodar("state", "--set", "RETIREABLE")
        self.rodar("retire", "--apply")
        os.environ["FORJA_AGENTES_TEST_TASK"] = "task-nova-depois-da-retirada-um"
        codigo, b = self.rodar("provision", "--branch", "b9", "--base", "main")
        self.assertEqual(codigo, 0, b)
        self.assertEqual(b["slot"], a["slot"])
        binding = json.loads((Path(b["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding["task_id"], "task-nova-depois-da-retirada-um")


class SegurancaDeCleanupTests(Base):
    def test_recusa_symlink_como_task_root(self) -> None:
        dados = self.provisionar()
        self.rodar("state", "--set", "RETIREABLE")
        raiz = Path(dados["task_root"])
        vitima = self.tmp / "vitima"
        vitima.mkdir()
        (vitima / "importante.txt").write_text("nao apague\n", encoding="utf-8")
        git(self.main, "worktree", "remove", "--force", str(raiz / "worktree"))
        shutil.rmtree(raiz)
        raiz.symlink_to(vitima)
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_NAO_ENCONTRADO)
        self.assertTrue((vitima / "importante.txt").exists())

    def test_nao_segue_symlink_para_fora_ao_apagar(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        fora = self.tmp / "fora"
        fora.mkdir()
        alvo = fora / "nao-apague.txt"
        alvo.write_text("preservar\n", encoding="utf-8")
        (raiz / "scratch" / "fuga").symlink_to(fora)
        self.rodar("state", "--set", "RETIREABLE")
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertFalse(raiz.exists())
        self.assertTrue(alvo.exists(), "o delete seguiu um symlink para fora do task root")
        self.assertTrue(fora.is_dir())

    def test_seal_nao_segue_symlink_para_fora(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        fora = self.tmp / "fora2"
        fora.mkdir()
        alvo = fora / "preservar.txt"
        alvo.write_text("x\n", encoding="utf-8")
        (raiz / "target" / "fuga").symlink_to(fora)
        codigo, resultado = self.rodar("seal", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertTrue(alvo.exists())

    def test_recusa_componente_symlink_no_caminho(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        # sanidade: um caminho sem symlink passa
        fa.sem_symlink_em_componentes(raiz / "target", self.main)
        (raiz / "target").rename(raiz / "target-real")
        (raiz / "target").symlink_to(raiz / "target-real")
        with self.assertRaises(fa.ForjaError):
            fa.sem_symlink_em_componentes(raiz / "target" / "x", self.main)
        # e o selo recusa em vez de apagar através do link
        codigo, resultado = self.rodar("seal", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, resultado)
        self.assertTrue((raiz / "target-real").is_dir())

    def test_nunca_apaga_o_checkout_canonico(self) -> None:
        dados = self.provisionar()
        with self.assertRaises(fa.ForjaError):
            fa.guardas_de_destruicao(self.main, self.main / "agentes", self.main, {"state": "RETIREABLE"})
        self.assertTrue((self.main / "README.md").exists())

    def test_nunca_apaga_a_propria_raiz_de_agentes(self) -> None:
        self.provisionar()
        with self.assertRaises(fa.ForjaError):
            fa.guardas_de_destruicao(
                self.main, self.main / "agentes", self.main / "agentes", {"state": "RETIREABLE"}
            )
        self.assertTrue((self.main / "agentes" / "README.md").exists())

    def test_recusa_caminho_fora_da_raiz_de_agentes(self) -> None:
        fora = self.tmp / "fora3"
        fora.mkdir()
        with self.assertRaises(fa.ForjaError):
            fa.guardas_de_destruicao(self.main, self.main / "agentes", fora, {"state": "RETIREABLE"})
        self.assertTrue(fora.is_dir())

    def test_recusa_traversal_no_task_id(self) -> None:
        for mau in ("../fuga", "a/b", "..", ".", "-x", ""):
            with self.assertRaises(fa.ForjaError):
                fa.validar_task(mau)

    def test_recusa_nome_de_slot_malformado(self) -> None:
        raiz = self.main / "agentes"
        intruso = raiz / "nao-e-slot"
        intruso.mkdir()
        with self.assertRaises(fa.ForjaError):
            fa.guardas_de_destruicao(self.main, raiz, intruso, {"state": "RETIREABLE"})
        self.assertTrue(intruso.is_dir())

    def test_bloqueia_quando_ha_processo_vivo_no_task_root(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        self.rodar("state", "--set", "RETIREABLE")
        proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], cwd=str(raiz / "worktree"))
        try:
            codigo, resultado = self.rodar("retire", "--apply")
            self.assertEqual(codigo, fa.EXIT_RECUSADO, resultado)
            self.assertEqual(resultado["status"], "BLOCKED_BY_PROCESS")
            self.assertTrue(raiz.is_dir())
        finally:
            proc.kill()
            proc.wait()


class VerificacaoTests(Base):
    def test_verify_aprova_layout_saudavel(self) -> None:
        self.provisionar()
        codigo, resultado = self.rodar("verify")
        self.assertEqual(codigo, 0, resultado)
        self.assertEqual(resultado["problems"], [])
        self.assertTrue(resultado["checks"]["provisioner_equals_finalizer"])
        self.assertTrue(resultado["checks"]["task_identity_ne_physical_path"])
        self.assertTrue(resultado["checks"]["task_roots_disjoint"])

    def test_verify_reprova_slot_sem_vinculo(self) -> None:
        self.provisionar()
        (self.main / "agentes" / "a02").mkdir()
        codigo, resultado = self.rodar("verify")
        self.assertEqual(codigo, 5)
        self.assertTrue(any("sem vínculo" in p for p in resultado["problems"]))

    def test_verify_reprova_metadata_orfa(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        shutil.rmtree(raiz / "worktree")
        codigo, resultado = self.rodar("verify")
        self.assertEqual(codigo, 5)
        self.assertTrue(resultado["checks"]["stale_git_worktree_metadata"])

    def test_verify_reprova_task_id_duplicado(self) -> None:
        self.provisionar()
        segundo = self.main / "agentes" / "a02"
        segundo.mkdir()
        (segundo / "task.json").write_text(
            json.dumps(
                {
                    "schema": fa.SCHEMA_BINDING,
                    "task_id": "issue-536-exemplo-de-identidade-longa",
                    "slot": "a02",
                }
            ),
            encoding="utf-8",
        )
        codigo, resultado = self.rodar("verify")
        self.assertEqual(codigo, 5)
        self.assertTrue(any("vinculado a" in p for p in resultado["problems"]))

    def test_observe_falha_quando_a_task_nao_tem_root(self) -> None:
        os.environ["FORJA_AGENTES_TEST_TASK"] = "task-que-nunca-foi-provisionada"
        codigo, resultado = self.rodar("observe")
        self.assertEqual(codigo, fa.EXIT_NAO_ENCONTRADO)
        self.assertEqual(resultado["status"], "NOT_FOUND")


class ModoDeTesteTests(Base):
    def test_override_sem_modo_de_teste_e_recusado(self) -> None:
        os.environ.pop("FORJA_AGENTES_TEST_MODE")
        try:
            with self.assertRaises(fa.ForjaError):
                fa.canonical_main()
        finally:
            os.environ["FORJA_AGENTES_TEST_MODE"] = "1"

    def test_raiz_de_producao_e_fixa(self) -> None:
        self.assertEqual(fa.CANONICAL_MAIN_PADRAO, "/pinker/repo/pinker-v0")


class PressaoDeCaminhoTests(Base):
    def test_slot_curto_preserva_orcamento_de_socket_unix(self) -> None:
        # O sandbox nativo constrói <repo_root>/target/pinker-exec/exec-<pid>-<n>/arvore/soquete.
        # O limite útil de sun_path é 107 bytes.
        producao = f"{fa.CANONICAL_MAIN_PADRAO}/{fa.AGENTES_DIRNAME}/a01/worktree"
        sufixo = "/target/pinker-exec/exec-1234567-99/arvore/soquete"
        self.assertLess(len(producao + sufixo), 108, f"{producao + sufixo} = {len(producao + sufixo)} bytes")

    def test_identidade_longa_nao_alonga_o_caminho_fisico(self) -> None:
        dados = self.provisionar()
        self.assertGreater(len(dados["task_id"]), 20)
        self.assertLessEqual(len(dados["slot"]), 5)


if __name__ == "__main__":
    unittest.main(verbosity=2)
