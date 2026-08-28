#!/usr/bin/env python3
"""Suíte da autoridade de layout `agentes/`.

Roda sem root, sem host da Forja e sem rede: todo o layout é montado num
diretório temporário sob modo de teste explícito. Os testes que importam são os
de recusa — um cleanup que só é testado no caminho feliz não prova nada.
"""

from __future__ import annotations

import ast
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

    def tornar_retiravel(self) -> None:
        """Leva a Task ao unico estado que autoriza destruicao.

        ACTIVE -> RETIREABLE nao existe de proposito: pular o selo apagaria a
        worktree e a memoria que a revisao ainda usa.
        """
        codigo, r = self.rodar("seal", "--apply")
        self.assertEqual(codigo, 0, r)
        codigo, r = self.rodar("state", "--set", "RETIREABLE")
        self.assertEqual(codigo, 0, r)

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

    def test_provision_nao_cria_nada_fora_do_root_nem_ao_falhar(self) -> None:
        """A propriedade não é "reprovar o path": é NÃO CRIAR fora do root.

        Um provisionamento que cria o diretório escapado e só depois reprova o
        caminho satisfaz qualquer asserção sobre a saída, e mesmo assim já
        sujou o filesystem. O que se assere aqui é o efeito colateral.
        """
        raiz = self.main / "agentes"
        antes = {p.name for p in raiz.iterdir()}
        codigo, dados = self.rodar("provision", "--branch", "b1", "--base", "main")
        depois = {p.name for p in raiz.iterdir()}
        novos = depois - antes
        # tudo que apareceu tem de ser slot bem formado, nada de `logs` solto
        for nome in novos:
            self.assertRegex(nome, r"^a[0-9]{2,4}$", f"provision criou algo fora do contrato: {nome}")
        # e nada pode ter sido criado acima da raiz de agentes
        for nome in ("logs", "target", "tmp", "scratch", "cache", "memory", "state", "artifacts"):
            self.assertFalse(
                (self.main / nome).exists(),
                f"provision criou {nome} FORA do task root, ao lado do checkout",
            )
        if codigo == 0:
            raizt = Path(dados["task_root"])
            for r in dados["resources"]:
                self.assertTrue(fa.contido_em(Path(r["path"]), raizt), r)

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
        self.tornar_retiravel()
        codigo, resultado = self.rodar("retire", "--apply")

        # A PROPRIEDADE primeiro, e lida sem estourar: `read_bytes` num arquivo
        # que a mutação apagou levanta FileNotFoundError ANTES de qualquer
        # assert, e o teste morreria pela exceção em vez de pela violação.
        self.assertTrue(marca.exists(), "retire(A) apagou a memória de B")
        self.assertEqual(marca.read_bytes(), antes, "retire(A) mutou a memória de B")
        self.assertTrue(Path(b["task_root"]).is_dir(), "retire(A) destruiu B")

        # só então o caminho feliz de A
        self.assertEqual(codigo, 0, resultado)
        self.assertTrue(resultado["task_root_absent"])
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
        self.tornar_retiravel()
        codigo, resultado = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, resultado)
        self.assertFalse(Path(dados["task_root"]).exists())
        self.assertEqual(resultado["stale_git_worktree_metadata"], [])
        saida = git(self.main, "worktree", "list", "--porcelain").stdout
        self.assertNotIn(str(Path(dados["task_root"]) / "worktree"), saida)

    def test_retire_nao_deixa_metadata_orfa(self) -> None:
        dados = self.provisionar()
        self.tornar_retiravel()
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
        self.tornar_retiravel()
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
        self.tornar_retiravel()
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
        # Nenhum codigo de saida e asserido antes do oraculo. A travessia
        # acontece no SELO — que limpa scratch — e o erro que ela levanta
        # depois de ja ter apagado o alvo externo derrubaria o teste pela causa
        # errada. O unico oraculo aqui e o arquivo de fora.
        self.rodar("seal", "--apply")
        self.rodar("state", "--set", "RETIREABLE")
        self.rodar("retire", "--apply")
        self.assertTrue(alvo.exists(), "o delete seguiu um symlink para fora do task root")
        self.assertTrue(fora.is_dir(), "o diretorio externo foi destruido pela travessia")
        # so depois, o caminho feliz: com o codigo correto tudo isso vale
        self.assertFalse(raiz.exists(), "o root deveria ter sido removido")

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

    def _binding_valido(self) -> dict:
        """Vínculo que passa dono e estado, para o teste alcançar a contenção.

        Sem isto a guarda de dono dispara antes e o teste passaria pelo motivo
        errado — verde enquanto a contenção estivesse quebrada.
        """
        return {
            "state": "RETIREABLE",
            "agent": fa.agente_corrente(),
            "task_id": "x",
            # o selo entra aqui porque a guarda de selo vem DEPOIS da contenção:
            # sem ele o teste pararia no selo e passaria pelo motivo errado,
            # que é exatamente a armadilha que este fixture existe para evitar
            "created_at": "2020-01-01T00:00:00Z",
            "sealed_at": "2020-01-02T00:00:00Z",
        }

    def test_nunca_apaga_o_checkout_candidato_recusa_por_contencao(self) -> None:
        """A recusa precisa ser POR CONTENÇÃO, não por um acaso do ambiente.

        Apontar as guardas para o checkout canônico levanta erro mesmo com a
        contenção desligada, porque o processo de teste tem cwd lá dentro e a
        guarda de processo dispara primeiro. Verde por acidente é o modo de
        falha que esta suíte existe para impedir, então o teste asserta o
        MOTIVO da recusa e não apenas que houve recusa.
        """
        self.provisionar()
        with self.assertRaises(fa.ForjaError) as ctx:
            fa.guardas_de_destruicao(
                self.main, self.main / "agentes", self.main, self._binding_valido()
            )
        msg = str(ctx.exception)
        self.assertNotIn("vínculo declara o agente", msg)
        self.assertIn("contido", msg, f"recusou, mas por outro motivo: {msg}")
        self.assertTrue((self.main / "README.md").exists())

    def test_nunca_apaga_a_propria_raiz_de_agentes(self) -> None:
        self.provisionar()
        with self.assertRaises(fa.ForjaError) as ctx:
            fa.guardas_de_destruicao(
                self.main, self.main / "agentes", self.main / "agentes", self._binding_valido()
            )
        # não pode parar na guarda de dono: o teste existe para a contenção
        self.assertNotIn("vínculo declara o agente", str(ctx.exception))
        self.assertTrue((self.main / "agentes" / "README.md").exists())

    def test_recusa_caminho_fora_da_raiz_de_agentes(self) -> None:
        fora = self.tmp / "fora3"
        fora.mkdir()
        with self.assertRaises(fa.ForjaError) as ctx:
            fa.guardas_de_destruicao(
                self.main, self.main / "agentes", fora, self._binding_valido()
            )
        self.assertNotIn("vínculo declara o agente", str(ctx.exception))
        self.assertTrue(fora.is_dir())

    def test_recusa_traversal_no_task_id(self) -> None:
        for mau in ("../fuga", "a/b", "..", ".", "-x", ""):
            with self.assertRaises(fa.ForjaError):
                fa.validar_task(mau)

    def test_recusa_nome_de_slot_malformado(self) -> None:
        raiz = self.main / "agentes"
        intruso = raiz / "nao-e-slot"
        intruso.mkdir()
        with self.assertRaises(fa.ForjaError) as ctx:
            fa.guardas_de_destruicao(self.main, raiz, intruso, self._binding_valido())
        self.assertIn("slot", str(ctx.exception))
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


class PropriedadeDaTaskTests(Base):
    """Regressoes dos blockers da revisao adversarial (F1, F2, F4).

    O ataque que estes testes fecham nao passa por caminho invalido: o caminho
    da Task B e perfeitamente valido. O que estava aberto era a interface, que
    aceitava `--task-id` como endereco em vez de como asserção.
    """

    def _provisionar_b(self) -> dict:
        os.environ["FORJA_AGENTES_TEST_TASK"] = "task-b-alvo-de-ataque-longa"
        codigo, b = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertEqual(codigo, 0, b)
        return b

    def test_retire_recusa_task_de_outrem_via_task_id(self) -> None:
        """B precisa estar no estado em que SÓ a identidade a protege.

        Com B em ACTIVE o gate de estado recusaria sozinho, e o teste passaria
        sem nunca exercitar a guarda de identidade — verde pelo motivo errado.
        Aqui B é legitimamente selada e RETIREABLE: todo gate menos o de
        identidade aprovaria a destruição.
        """
        a = self.provisionar("b1")
        b = self._provisionar_b()
        self.assertEqual(self.rodar("seal", "--apply")[0], 0)
        self.assertEqual(self.rodar("state", "--set", "RETIREABLE")[0], 0)
        binding = json.loads((Path(b["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding["state"], "RETIREABLE")
        self.assertIn("sealed_at", binding)

        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        codigo, r = self.rodar("retire", "--task-id", b["task_id"], "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("não é a Task ativa do chamador", r["error"])
        self.assertTrue(Path(b["task_root"]).is_dir(), "A retirou o root de B")
        self.assertTrue((Path(b["task_root"]) / "memory").is_dir(), "A tocou a memória de B")

    def test_seal_recusa_task_de_outrem_via_task_id(self) -> None:
        a = self.provisionar("b1")
        b = self._provisionar_b()
        peso = Path(b["task_root"]) / "target" / "peso.bin"
        peso.write_bytes(b"b" * 2048)
        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        codigo, r = self.rodar("seal", "--task-id", b["task_id"], "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertTrue(peso.exists(), "A selou o target de B")

    def test_state_recusa_task_de_outrem_via_task_id(self) -> None:
        a = self.provisionar("b1")
        b = self._provisionar_b()
        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        codigo, r = self.rodar("state", "--task-id", b["task_id"], "--set", "SEALED")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        binding = json.loads((Path(b["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding.get("state"), "ACTIVE")

    def test_observe_com_task_id_alheio_continua_permitido(self) -> None:
        a = self.provisionar("b1")
        b = self._provisionar_b()
        os.environ["FORJA_AGENTES_TEST_TASK"] = a["task_id"]
        codigo, r = self.rodar("observe", "--task-id", b["task_id"])
        self.assertEqual(codigo, 0, r)  # ler nao muta: continua liberado
        self.assertEqual(r["task_root"], b["task_root"])

    def test_task_id_proprio_explicito_e_aceito(self) -> None:
        a = self.provisionar("b1")
        codigo, r = self.rodar("state", "--task-id", a["task_id"], "--set", "REVIEW")
        self.assertEqual(codigo, 0, r)

    def test_mutacao_recusa_root_de_outro_agente(self) -> None:
        dados = self.provisionar("b1")
        binding = json.loads((Path(dados["task_root"]) / "task.json").read_text(encoding="utf-8"))
        binding["agent"] = "outra-agente"
        (Path(dados["task_root"]) / "task.json").write_text(
            json.dumps(binding), encoding="utf-8"
        )
        codigo, r = self.rodar("state", "--set", "SEALED")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("pertence ao agente", r["error"])


class TransicaoDeEstadoTests(Base):
    def test_active_nao_salta_direto_para_retireable(self) -> None:
        self.provisionar()
        codigo, r = self.rodar("state", "--set", "RETIREABLE")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("transição não autorizada", r["error"])

    def test_caminho_autorizado_ate_a_destruicao_passa_pelo_selo(self) -> None:
        self.provisionar()
        self.assertEqual(self.rodar("seal", "--apply")[0], 0)
        self.assertEqual(self.rodar("state", "--set", "RETIREABLE")[0], 0)
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, 0, r)

    def test_sealed_pode_reabrir_para_correcao(self) -> None:
        self.provisionar()
        self.rodar("seal", "--apply")
        codigo, r = self.rodar("state", "--set", "FIX_REQUIRED")
        self.assertEqual(codigo, 0, r)
        # e dali nao se destroi sem selar de novo
        self.assertEqual(self.rodar("state", "--set", "RETIREABLE")[0], fa.EXIT_RECUSADO)

    def test_retire_exige_retireable_e_nao_apenas_sealed(self) -> None:
        self.provisionar()
        self.rodar("seal", "--apply")
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("RETIREABLE", r["error"])


class SeloForjadoTests(Base):
    """Round 2 F2: o selo precisa ser um fato ocorrido, nao um rotulo escrito."""

    def test_state_nao_atribui_sealed(self) -> None:
        self.provisionar()
        codigo, r = self.rodar("state", "--set", "SEALED")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("não é atribuível por `state`", r["error"])

    def test_retire_recusa_rotulo_retireable_sem_selo_ocorrido(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        # forja o vinculo a mao: rotulo terminal sem nenhum selo por tras
        binding = json.loads((raiz / "task.json").read_text(encoding="utf-8"))
        binding["state"] = "RETIREABLE"
        (raiz / "task.json").write_text(json.dumps(binding), encoding="utf-8")
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("sem evidência de EXECUTION_SEAL", r["error"])
        self.assertTrue(raiz.is_dir(), "destruiu uma Task cujo selo nunca ocorreu")

    def test_retire_recusa_evidencia_de_selo_malformada(self) -> None:
        """A chave existir não basta: `sealed_at: "x"` não é um selo."""
        dados = self.provisionar()
        self.rodar("seal", "--apply")
        self.rodar("state", "--set", "RETIREABLE")
        raiz = Path(dados["task_root"])
        binding = json.loads((raiz / "task.json").read_text(encoding="utf-8"))
        binding["sealed_at"] = "x"
        (raiz / "task.json").write_text(json.dumps(binding), encoding="utf-8")
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("malformado", r["error"])
        self.assertTrue(raiz.is_dir())

    def test_retire_recusa_created_at_ausente_ou_ilegivel(self) -> None:
        """Ramos que antes autorizavam por omissão: `created_at` ausente pulava
        a comparação e um `created_at` ilegível caía num `pass`."""
        for mutacao in ({"remover": "created_at"}, {"created_at": "x"}, {"created_at": 12345}):
            with self.subTest(mutacao=mutacao):
                dados = self.provisionar()
                self.rodar("seal", "--apply")
                self.rodar("state", "--set", "RETIREABLE")
                raiz = Path(dados["task_root"])
                b = json.loads((raiz / "task.json").read_text(encoding="utf-8"))
                if "remover" in mutacao:
                    b.pop("created_at", None)
                else:
                    b.update(mutacao)
                (raiz / "task.json").write_text(json.dumps(b), encoding="utf-8")
                codigo, r = self.rodar("retire", "--apply")
                self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
                self.assertIn("created_at", r["error"])
                self.assertTrue(raiz.is_dir())
                self.tearDown(); self.setUp()

    def test_retire_recusa_carimbo_nao_canonico(self) -> None:
        """`strptime` aceita segundo 61 e formas não canônicas; o carimbo que
        autoriza destruição precisa sobreviver a um round-trip."""
        for ruim in ("2026-08-28T05:48:61Z", "2026-8-28T05:48:59Z", "2026-08-28T05:48:59"):
            with self.subTest(carimbo=ruim):
                with self.assertRaises(fa.ForjaError):
                    fa.instante_canonico(ruim, "sealed_at")
        self.assertIsNotNone(fa.instante_canonico("2026-08-28T05:48:59Z", "sealed_at"))

    def test_retire_recusa_selo_anterior_a_criacao_do_root(self) -> None:
        dados = self.provisionar()
        self.rodar("seal", "--apply")
        self.rodar("state", "--set", "RETIREABLE")
        raiz = Path(dados["task_root"])
        binding = json.loads((raiz / "task.json").read_text(encoding="utf-8"))
        binding["sealed_at"] = "2000-01-01T00:00:00Z"
        (raiz / "task.json").write_text(json.dumps(binding), encoding="utf-8")
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("antes da criação", r["error"])
        self.assertTrue(raiz.is_dir())

    def test_selo_real_deixa_evidencia_datada(self) -> None:
        dados = self.provisionar()
        codigo, r = self.rodar("seal", "--apply")
        self.assertEqual(codigo, 0, r)
        binding = json.loads((Path(dados["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding["state"], "SEALED")
        self.assertIn("sealed_at", binding)
        self.assertIn("sealed_reclaimed_bytes", binding)


class ProvisionEMutanteTests(Base):
    """Round 2 F1: provisionar cria worktree e branch, logo e mutante."""

    def test_provision_recusa_task_id_alheio(self) -> None:
        self.provisionar("b1")
        codigo, r = self.rodar(
            "provision", "--task-id", "task-alheia-nao-provisionada", "--branch", "bx"
        )
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertIn("não é a Task ativa do chamador", r["error"])
        self.assertFalse((self.main / "agentes" / "a02").exists())


class MountFailClosedTests(Base):
    """Round 2 F4: mountinfo ilegivel nao pode certificar ausencia de mount."""

    def test_mountinfo_ilegivel_bloqueia_em_vez_de_certificar(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        original = fa.Path

        class PathQueFalhaNoMountinfo(type(Path())):
            def read_text(self, *a, **k):
                if str(self) == "/proc/self/mountinfo":
                    raise OSError("ilegivel de proposito")
                return super().read_text(*a, **k)

        alvo = fa.__dict__["Path"]
        fa.__dict__["Path"] = PathQueFalhaNoMountinfo
        try:
            with self.assertRaises(fa.ForjaError) as ctx:
                fa.mountpoints()
            self.assertIn("mount", str(ctx.exception).lower())
        finally:
            fa.__dict__["Path"] = alvo
        self.assertTrue(raiz.is_dir())


class FalhaDeGitTests(Base):
    """F2: falha do Git nao pode virar sucesso terminal."""

    def test_retire_falha_quando_o_desregistro_falha(self) -> None:
        dados = self.provisionar()
        self.tornar_retiravel()
        raiz = Path(dados["task_root"])
        original = fa.git

        def git_quebrado(main, *args, check=True):
            if args[:2] == ("worktree", "remove"):
                class R:
                    returncode = 1
                    stdout = ""
                    stderr = "falha simulada de desregistro"
                return R()
            return original(main, *args, check=check)

        fa.git = git_quebrado
        try:
            codigo, r = self.rodar("retire", "--apply")
        finally:
            fa.git = original
        self.assertEqual(codigo, fa.EXIT_FALHA, r)
        self.assertEqual(r["status"], "FAILED")
        self.assertTrue(raiz.is_dir(), "removeu o root apesar de o desregistro falhar")

    def test_identidade_e_conferida_antes_da_primeira_remocao(self) -> None:
        """O Git resolve o caminho por texto: a primeira remoção também precisa
        da identidade fixada, não apenas a segunda."""
        dados = self.provisionar()
        self.tornar_retiravel()
        raiz = Path(dados["task_root"])
        identidade_falsa = (0, 0)
        with self.assertRaises(fa.ForjaError) as ctx:
            fa.confirmar_identidade(raiz, identidade_falsa)
        self.assertIn("identidade do alvo mudou", str(ctx.exception))
        # e a identidade real passa
        st = raiz.lstat()
        fa.confirmar_identidade(raiz, (st.st_dev, st.st_ino))

    def test_falha_de_desregistro_reporta_o_estado_real_e_nao_presume(self) -> None:
        """Medido: um subdiretório 0500 faz o Git sair 255 DEPOIS de apagar o
        registro. Dizer "nada foi removido" ali é o inverso do que aconteceu."""
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        self.tornar_retiravel()
        sub = raiz / "worktree" / "sub"
        sub.mkdir()
        (sub / "arquivo.txt").write_text("dado\n", encoding="utf-8")
        os.chmod(sub, 0o500)
        try:
            codigo, r = self.rodar("retire", "--apply")
            self.assertEqual(codigo, fa.EXIT_FALHA, r)
            msg = r["error"]
            # o relatorio emite OBSERVAVEIS, nao conclusao
            self.assertIn("estado observado:", msg)
            self.assertIn("registro_presente=", msg)
            self.assertIn("worktree_no_disco=", msg)
            self.assertNotIn("Nada foi removido", msg)
            # e o verify precisa ENXERGAR o resíduo, não declarar tudo limpo
            codigo_v, v = self.rodar("verify")
            if (raiz / "worktree").exists():
                self.assertEqual(codigo_v, 5, v)
                self.assertTrue(
                    v["checks"]["unregistered_worktree_dirs"],
                    "worktree órfã no disco ficou invisível para verify",
                )
        finally:
            os.chmod(sub, 0o700)

    def test_inspecao_de_metadata_que_falha_e_erro_nao_lista_vazia(self) -> None:
        self.provisionar()
        original = fa.git

        def git_quebrado(main, *args, check=True):
            if args[:1] == ("rev-parse",) and "--git-common-dir" in args:
                class R:
                    returncode = 128
                    stdout = ""
                    stderr = "nao foi possivel inspecionar"
                return R()
            return original(main, *args, check=check)

        fa.git = git_quebrado
        try:
            with self.assertRaises(fa.ForjaError):
                fa.metadata_orfa(self.main)
        finally:
            fa.git = original


class ReivindicacaoDeSlotTests(Base):
    """F4: a alocacao de slot precisa ser atomica, nao scan-then-use."""

    def test_reivindicar_deixa_marca_para_o_proximo_chamador(self) -> None:
        """Reivindicar precisa ser observável, não apenas planejado.

        Este teste é determinístico de propósito. A primeira versão usava oito
        threads e uma barreira, e detectava a corrida só às vezes: passou local
        e no primeiro CI, e ficou verde com a mutação aplicada num segundo CI do
        MESMO commit. Um detector probabilístico não é um gate — ele apenas
        adia o vermelho para um momento pior.

        A propriedade que importa não precisa de concorrência para ser
        expressa: se reivindicar um slot não deixa marca, o próximo chamador
        reivindica o mesmo. Duas chamadas seguidas bastam.
        """
        raiz = self.main / "agentes"
        primeiro = fa.reivindicar_slot(raiz)
        segundo = fa.reivindicar_slot(raiz)
        self.assertNotEqual(
            primeiro,
            segundo,
            "reivindicar nao deixou marca: o segundo chamador recebeu o mesmo slot",
        )
        self.assertTrue((raiz / primeiro).is_dir(), "o slot reivindicado precisa existir ja")
        self.assertTrue((raiz / segundo).is_dir())

    def test_reivindicar_pula_slot_criado_por_terceiro_no_meio(self) -> None:
        """Um concorrente que vence a corrida não pode ser sobrescrito.

        O efeito colateral é injetado em `slots_existentes`, que é o ponto por
        onde as duas implementações passam: a varredura devolve o mundo antigo
        enquanto o disco já mudou. É exatamente a janela entre olhar e criar.
        """
        raiz = self.main / "agentes"
        original = fa.slots_existentes

        def varredura_obsoleta(r: Path):
            resultado = original(r)
            alvo = r / "a01"
            if not alvo.exists():
                os.mkdir(alvo, 0o2770)  # o concorrente venceu aqui
                (alvo / "task.json").write_text(
                    json.dumps(
                        {"schema": fa.SCHEMA_BINDING, "task_id": "task-do-concorrente", "slot": "a01"}
                    ),
                    encoding="utf-8",
                )
            return resultado  # devolve o mundo de antes, de proposito

        fa.slots_existentes = varredura_obsoleta
        try:
            obtido = fa.reivindicar_slot(raiz)
        finally:
            fa.slots_existentes = original
        self.assertNotEqual(obtido, "a01", "sobrescreveu o slot de quem venceu a corrida")
        binding = json.loads((raiz / "a01" / "task.json").read_text(encoding="utf-8"))
        self.assertEqual(binding["task_id"], "task-do-concorrente", "vinculo do concorrente foi perdido")

    def test_reprovisionar_reabre_e_descarta_o_selo_antigo(self) -> None:
        """Um root reprovisionado não pode continuar destrutível.

        Herdar `RETIREABLE` deixaria a Task recém-reaberta a um `retire` de
        distância, e herdar `sealed_at` faria um selo antigo autorizar a
        destruição de trabalho novo.
        """
        dados = self.provisionar("b1")
        self.rodar("seal", "--apply")
        self.rodar("state", "--set", "RETIREABLE")
        codigo, r = self.rodar("provision")
        self.assertEqual(codigo, 0, r)
        self.assertEqual(r["state"], "ACTIVE")
        self.assertEqual(r["reopened_from"], "RETIREABLE")
        binding = json.loads((Path(dados["task_root"]) / "task.json").read_text(encoding="utf-8"))
        self.assertNotIn("sealed_at", binding, "selo antigo sobreviveu à reabertura")
        codigo, r = self.rodar("retire", "--apply")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)

    def test_provision_nao_adota_root_sem_vinculo(self) -> None:
        intruso = self.main / "agentes" / "a01"
        intruso.mkdir()
        codigo, r = self.rodar("provision", "--branch", "b1", "--base", "main")
        self.assertEqual(codigo, 0, r)
        # nao pode ter adotado a01: ele nao tinha vinculo desta Task
        self.assertNotEqual(r["slot"], "a01")


class IdentidadeDoAlvoTests(Base):
    """Round 6 F3: as guardas de identidade não tinham teste de comportamento.

    Mutantes que removiam as chamadas de `confirmar_identidade` passavam pela
    suíte inteira. Aqui o alvo é REALMENTE trocado entre a guarda e a remoção,
    injetando o swap num ponto por onde o código passa obrigatoriamente, e a
    operação precisa recusar.
    """

    def test_retire_recusa_quando_a_worktree_e_trocada_apos_a_guarda(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        self.tornar_retiravel()
        wt = raiz / "worktree"
        impostor = raiz / "impostor"
        impostor.mkdir()
        (impostor / "prova.txt").write_text("nao sou a worktree\n", encoding="utf-8")

        original = fa.worktree_registrada
        trocou = {"feito": False}

        def troca_no_meio(main, caminho):
            # chamado entre a fixação da identidade e o `git worktree remove`
            r = original(main, caminho)
            if not trocou["feito"] and Path(caminho) == wt:
                trocou["feito"] = True
                wt.rename(raiz / "worktree-original")
                impostor.rename(wt)
            return r

        fa.worktree_registrada = troca_no_meio
        try:
            codigo, r = self.rodar("retire", "--apply")
        finally:
            fa.worktree_registrada = original
        self.assertTrue(trocou["feito"], "a injeção não ocorreu; o teste não provaria nada")
        # A propriedade: o objeto que ocupou o nome depois da guarda NÃO é
        # removido. Honestidade sobre o mecanismo: nesta troca o próprio Git
        # também recusa, porque o impostor não tem `.git` válido. A guarda de
        # identidade é defesa em profundidade sobre essa validação, e este
        # teste prova a propriedade, não a atribui a uma camada específica.
        self.assertNotEqual(codigo, 0, r)
        self.assertTrue((wt / "prova.txt").exists(), "removeu o impostor no lugar do alvo aprovado")

    def test_seal_recusa_quando_o_recurso_e_trocado_apos_a_guarda(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        alvo = raiz / "target"
        (alvo / "peso.bin").write_bytes(b"z" * 4096)
        impostor = raiz / "impostor"
        impostor.mkdir()
        (impostor / "prova.txt").write_text("nao sou o target\n", encoding="utf-8")

        original = fa.processos_no_root
        trocou = {"feito": False}

        def troca_no_meio(caminho):
            # chamado depois de fixar a identidade e antes de remover
            r = original(caminho)
            if not trocou["feito"] and Path(caminho) == alvo:
                trocou["feito"] = True
                alvo.rename(raiz / "target-original")
                impostor.rename(alvo)
            return r

        fa.processos_no_root = troca_no_meio
        try:
            codigo, r = self.rodar("seal", "--apply")
        finally:
            fa.processos_no_root = original
        self.assertTrue(trocou["feito"], "a injeção não ocorreu; o teste não provaria nada")
        self.assertEqual(codigo, fa.EXIT_RECUSADO, r)
        self.assertTrue((alvo / "prova.txt").exists(), "o selo removeu o impostor")


class DeteccaoDeResiduoTests(Base):
    """Round 6 F3: os detectores de resíduo não tinham teste próprio.

    Revertê-los ao fail-open não quebrava nada — um detector sem teste é um
    comentário.
    """

    def test_worktree_com_git_ilegivel_e_reportada_e_nao_ignorada(self) -> None:
        dados = self.provisionar()
        wt = Path(dados["task_root"]) / "worktree"
        marcador = wt / ".git"
        os.chmod(wt, 0o000)
        try:
            achados = fa.worktrees_desregistradas(self.main, self.main / "agentes")
            self.assertTrue(achados, "worktree com .git ilegível desapareceu do detector")
            self.assertIn(str(wt), " ".join(achados))
        finally:
            os.chmod(wt, 0o755)

    def test_symlink_no_lugar_da_worktree_e_achado(self) -> None:
        dados = self.provisionar()
        raiz = Path(dados["task_root"])
        wt = raiz / "worktree"
        git(self.main, "worktree", "remove", "--force", str(wt))
        fora = self.tmp / "outro"
        fora.mkdir()
        wt.symlink_to(fora)
        achados = fa.worktrees_desregistradas(self.main, self.main / "agentes")
        self.assertTrue(any("symlink" in a for a in achados), f"symlink ignorado: {achados}")

    def test_slot_ilegivel_e_achado_e_nao_pulado(self) -> None:
        """Cobre o ramo em que o lstat da PRÓPRIA worktree falha.

        O teste do `.git` ilegível exercita outro `except`. Cada ramo de erro
        precisa do seu próprio oráculo — foi assim que a regressão do detector
        sobreviveu à suíte inteira.
        """
        dados = self.provisionar()
        wt = Path(dados["task_root"]) / "worktree"
        real_lstat = Path.lstat

        def lstat_que_nega(self, *a, **k):
            if self == wt:
                raise PermissionError(13, "Permission denied")
            return real_lstat(self, *a, **k)

        Path.lstat = lstat_que_nega
        try:
            achados = fa.worktrees_desregistradas(self.main, self.main / "agentes")
        finally:
            Path.lstat = real_lstat
        self.assertTrue(achados, "worktree ilegível desapareceu do detector de resíduo")
        self.assertIn("ilegível", " ".join(achados))

    def test_base_de_metadata_ilegivel_levanta(self) -> None:
        """Cobre o ramo do lstat da base, distinto do ramo da listagem."""
        self.provisionar()
        base = self.main / ".git" / "worktrees"
        real_lstat = Path.lstat

        def lstat_que_nega(self, *a, **k):
            if self == base:
                raise PermissionError(13, "Permission denied")
            return real_lstat(self, *a, **k)

        Path.lstat = lstat_que_nega
        try:
            with self.assertRaises(fa.ForjaError) as ctx:
                fa.metadata_orfa(self.main)
            # o motivo tem de ser a leitura da metadata, nao um erro do git
            self.assertIn("metadata de worktree", str(ctx.exception))
        finally:
            Path.lstat = real_lstat

    def test_listagem_de_metadata_que_falha_levanta(self) -> None:
        """Ramo distinto do lstat: o diretório existe e a LISTAGEM falha."""
        self.provisionar()
        base = self.main / ".git" / "worktrees"
        real_iterdir = Path.iterdir

        def iterdir_que_nega(self, *a, **k):
            if self == base:
                raise PermissionError(13, "Permission denied")
            return real_iterdir(self, *a, **k)

        Path.iterdir = iterdir_que_nega
        try:
            with self.assertRaises(fa.ForjaError) as ctx:
                fa.metadata_orfa(self.main)
            self.assertIn("ilistável", str(ctx.exception))
        finally:
            Path.iterdir = real_iterdir

    def test_metadata_ilegivel_levanta_em_vez_de_devolver_vazio(self) -> None:
        self.provisionar()
        comum = self.main / ".git" / "worktrees"
        self.assertTrue(comum.is_dir())
        os.chmod(comum, 0o000)
        try:
            with self.assertRaises(fa.ForjaError):
                fa.metadata_orfa(self.main)
        finally:
            os.chmod(comum, 0o755)


class PredicadoBooleanoTests(Base):
    """Gate estrutural contra a classe de defeito que ja apareceu nove vezes.

    Historico curto da propria DEFESA, que e o que este bloco documenta:

    - rodada  8: o gate so via `obj.exists()` literal.
    - rodada 10: o gate cobria SETE FUNCOES POR NOME; helper externo,
      `getattr` dinamico e `attrgetter` passavam, e tres provas reais do
      caminho destrutivo nunca estiveram na lista.
    - rodada 11: a correcao da 10 trocou a lista por um FECHO TRANSITIVO, e o
      fecho tambem foi furado — ele so seguia `ast.Call(func=ast.Name)`, entao
      `ALIAS_EXISTE = Path.exists` no nivel do modulo entrava na decisao
      destrutiva por fora dele.

    Tres escopos, tres fugas. O padrao e o escopo: toda tentativa de dizer
    "estas funcoes aqui sao as criticas" foi contornada por uma forma de
    alcancar a funcao que o escopo nao modelava. Entao o escopo deixa de ser
    calculado: e O MODULO INTEIRO, com isencoes fechadas e verificadas.

    Nao ha mais fecho para escapar, nem lista para envelhecer.
    """

    # Nomes de predicado que devolvem False tanto para "nao e" quanto para
    # "nao consegui olhar". A lista cobre `pathlib` E `os.path`: nenhum
    # `os.path.isdir` aparece hoje na ferramenta, e e por isso que entra
    # agora — proibir antes do primeiro uso custa uma linha; depois do
    # primeiro uso custa uma rodada adversarial.
    PROIBIDOS = {
        "exists", "is_file", "is_symlink", "is_dir",
        "isdir", "isfile", "islink", "lexists",
        "is_mount", "is_block_device", "is_char_device", "is_fifo",
        "is_socket", "is_junction", "access",
    }
    # Acesso dinamico a atributo: nao da para provar QUAL atributo e lido, e
    # o que nao se prova, num modulo que apaga diretorio, se recusa.
    DINAMICOS = {"__getattribute__", "__getattr__", "attrgetter"}

    # VAZIA. A rodada 12 mostrou que a guarda de isencao era insustentavel:
    # `descrever_recurso` levantava ForjaError TRANSITIVAMENTE (via
    # `exigir_contido`) e a prova, que so via `raise` direto, aprovava; e
    # `medir` engolia OSError e subcontava bytes do relatorio. As duas
    # isentas eram, cada uma a seu modo, a mesma classe de defeito.
    #
    # Em vez de tornar a guarda mais esperta — terceira tentativa de regra
    # esperta nesta ferramenta — as duas passaram a usar os helpers
    # tri-state. Sem isencao nao ha guarda de isencao para contornar.
    ISENTAS: dict[str, str] = {}

    @staticmethod
    def _funcoes(arvore):
        return {n.name: n for n in ast.walk(arvore) if isinstance(n, ast.FunctionDef)}

    @classmethod
    def _liga_a_proibido(cls, valor) -> bool:
        """O valor atribuido e um predicado proibido (ou um acesso indecidivel)?"""
        if isinstance(valor, ast.Attribute):
            return valor.attr in cls.PROIBIDOS or valor.attr in cls.DINAMICOS
        if (
            isinstance(valor, ast.Call)
            and isinstance(valor.func, ast.Name)
            and valor.func.id == "getattr"
            and len(valor.args) >= 2
        ):
            alvo = valor.args[1]
            return not isinstance(alvo, ast.Constant) or alvo.value in cls.PROIBIDOS
        return False

    @classmethod
    def _aliases(cls, arvore):
        """Nomes ligados a um predicado proibido em QUALQUER lugar do modulo.

        Foi por aqui que a rodada 11 passou: a ligacao morava no nivel do
        modulo, fora de toda funcao, e no ponto de uso so aparecia um
        `ast.Name`. Rastrear a ligacao onde ela acontece e mais barato do que
        adivinhar o ponto de uso.
        """
        achados = set()
        # Ponto fixo: `_A1 = Path.exists` e depois `_A2 = _A1` ligam o mesmo
        # predicado em dois passos, e a rodada 12 passou pelo gate com essa
        # cadeia. Repetir a varredura ate estabilizar custa microssegundos e
        # fecha a cadeia de qualquer comprimento.
        for _ in range(len(list(ast.walk(arvore)))):
            antes = len(achados)
            achados |= cls._passo_de_alias(arvore, achados)
            if len(achados) == antes:
                break
        return achados

    @classmethod
    def _passo_de_alias(cls, arvore, conhecidos):
        achados = set()
        for no in ast.walk(arvore):
            # `from os.path import isdir as _existe_dir` liga o nome sem
            # nenhum Attribute e sem nenhum Assign: no ponto de uso so ha um
            # ast.Name. O oraculo comportamental pegou essa forma, mas so
            # porque a funcao afetada tinha oraculo; o tripwire tem de pegar
            # sozinho, e importar e uma ligacao como outra qualquer.
            if isinstance(no, ast.ImportFrom):
                for apelido in no.names:
                    if apelido.name in cls.PROIBIDOS:
                        achados.add(apelido.asname or apelido.name)
                continue
            def ligado(valor):
                if valor is None:
                    return False
                if isinstance(valor, ast.Name) and valor.id in conhecidos:
                    return True
                return cls._liga_a_proibido(valor)

            alvos = []
            if isinstance(no, ast.Assign) and ligado(no.value):
                alvos = no.targets
            elif isinstance(no, ast.AnnAssign) and ligado(no.value):
                alvos = [no.target]
            for alvo in alvos:
                if isinstance(alvo, ast.Name):
                    achados.add(alvo.id)
        return achados

    def test_modulo_inteiro_nao_usa_predicado_booleano_de_existencia(self) -> None:
        """Escopo = modulo inteiro menos isencoes fechadas.

        Pega chamada direta, alias por atribuicao (em qualquer escopo),
        `getattr` constante, `getattr` de nome montado em runtime,
        `attrgetter` e `__getattribute__`.
        """
        arvore = ast.parse(FONTE.read_text(encoding="utf-8"))
        funcs = self._funcoes(arvore)
        aliases = self._aliases(arvore)
        ofensas = []
        for nome in sorted(funcs):
            if nome in self.ISENTAS:
                continue
            for interno in ast.walk(funcs[nome]):
                if isinstance(interno, ast.Attribute):
                    if interno.attr in self.PROIBIDOS:
                        ofensas.append(f"{nome}:{interno.lineno} referencia .{interno.attr}")
                    elif interno.attr in self.DINAMICOS:
                        ofensas.append(f"{nome}:{interno.lineno} acesso dinamico .{interno.attr}")
                elif isinstance(interno, ast.Name) and interno.id in aliases:
                    ofensas.append(f"{nome}:{interno.lineno} usa alias {interno.id!r} de predicado proibido")
                elif (
                    isinstance(interno, ast.Call)
                    and isinstance(interno.func, ast.Name)
                    and interno.func.id == "getattr"
                    and len(interno.args) >= 2
                ):
                    alvo = interno.args[1]
                    if isinstance(alvo, ast.Constant):
                        if alvo.value in self.PROIBIDOS:
                            ofensas.append(f"{nome}:{interno.lineno} getattr(..., {alvo.value!r})")
                    else:
                        ofensas.append(f"{nome}:{interno.lineno} getattr com nome dinamico")
        self.assertEqual(
            ofensas,
            [],
            "predicado booleano de existencia fora das isencoes; use "
            "estado_do_caminho()/e_symlink()/e_diretorio(), que nao conseguem "
            "representar 'nao olhei' como False:\n" + "\n".join(ofensas),
        )

    def test_nao_existe_isencao(self) -> None:
        """A allowlist tem de continuar vazia.

        Foram tres tentativas de regra esperta nesta ferramenta — lista de
        nomes, fecho transitivo, guarda de isencao declarativa — e as tres
        foram contornadas. Uma isencao nova precisa de decisao humana
        explicita, nao de uma linha a mais num dicionario.
        """
        self.assertEqual(
            self.ISENTAS,
            {},
            "isencao reintroduzida: prove por ORACULO COMPORTAMENTAL que a "
            "funcao nao decide, em vez de declara-lo",
        )

    def test_modulo_nao_engole_a_recusa_do_tri_state(self) -> None:
        """Tripwire sintatico, NAO prova.

        `any(raise)`/`any(.append)` aceitava `raise` morto sob `if False` e
        `append` em lista descartada, e recusava `return [*achados, msg]`, que
        e correto. Sintaxe nao decide se o erro chega a quem chamou; quem
        decide isso e `test_inspecao_que_falha_aparece_no_resultado`.

        O que sobra aqui e barato e util: recusar handler largo demais e
        handler que so segue em frente.
        """
        arvore = ast.parse(FONTE.read_text(encoding="utf-8"))
        ofensas = []
        for no in ast.walk(arvore):
            if not isinstance(no, ast.ExceptHandler):
                continue
            if no.type is None:
                ofensas.append(f"linha {no.lineno}: except nu")
                continue
            nomes = []
            if isinstance(no.type, ast.Name):
                nomes = [no.type.id]
            elif isinstance(no.type, ast.Tuple):
                nomes = [e.id for e in no.type.elts if isinstance(e, ast.Name)]
            if {"Exception", "BaseException"} & set(nomes):
                ofensas.append(f"linha {no.lineno}: except {'/'.join(nomes)} largo demais")
            elif "ForjaError" in nomes and all(
                isinstance(c, (ast.Pass, ast.Continue)) for c in no.body
            ):
                ofensas.append(f"linha {no.lineno}: engole ForjaError sem registrar")
        self.assertEqual(
            ofensas,
            [],
            "tratamento de erro largo ou vazio:\n" + "\n".join(ofensas),
        )

    def test_inspecao_que_falha_aparece_no_resultado(self) -> None:
        """O oraculo COMPORTAMENTAL da recusa tri-state.

        A rodada 11 mostrou que ele era incompleto: nao provava que o produtor
        tinha sido CHAMADO, entao um consumidor que nunca chamasse
        `worktree_registrada` e reportasse tudo como residuo passava. Agora ha
        sentinela de chamada e controle positivo.
        """
        dados = self.provisionar()
        wt = Path(dados["task_root"]) / "worktree"
        raiz = self.main / "agentes"
        original = fa.worktree_registrada

        # controle positivo: worktree REGISTRADA nao pode aparecer como residuo
        self.assertNotIn(
            str(wt),
            " ".join(fa.worktrees_desregistradas(self.main, raiz)),
            "worktree registrada reportada como residuo: o detector nao consulta o registro",
        )

        chamadas = []

        def recusa(main, alvo):
            chamadas.append(alvo)
            raise fa.ForjaError("DENIED", "registro de worktree inobservavel")

        fa.worktree_registrada = recusa
        try:
            achados = fa.worktrees_desregistradas(self.main, raiz)
        finally:
            fa.worktree_registrada = original

        self.assertTrue(chamadas, "o detector nem chamou worktree_registrada: o oraculo seria tautologia")
        self.assertTrue(achados, "inspecao que falhou sumiu do resultado: a recusa foi engolida")
        self.assertIn(str(wt), " ".join(achados))

    def test_task_root_inobservavel_bloqueia_a_destruicao(self) -> None:
        """O oraculo COMPORTAMENTAL do predicado de existencia.

        A rodada 11 mostrou que ele passava pelo motivo errado: `Path.lstat`
        cego derrubava `sem_symlink_em_componentes` ANTES da decisao sob
        teste, entao o teste ficava verde mesmo com a decisao mutada. Aqui a
        prova de symlink e neutralizada para que a proxima leitura de estado
        seja exatamente a que se quer observar.
        """
        dados = self.provisionar()
        task_root = Path(dados["task_root"])
        raiz = self.main / "agentes"

        real_lstat = Path.lstat
        real_sem_symlink = fa.sem_symlink_em_componentes

        def lstat_cego(self, *a, **k):
            if str(self) == str(task_root):
                raise PermissionError(13, "Permission denied")
            return real_lstat(self, *a, **k)

        fa.sem_symlink_em_componentes = lambda *a, **k: None
        Path.lstat = lstat_cego
        try:
            with self.assertRaises(fa.ForjaError) as ctx:
                fa.guardas_de_destruicao(self.main, raiz, task_root, None)
        finally:
            Path.lstat = real_lstat
            fa.sem_symlink_em_componentes = real_sem_symlink

        self.assertIn(
            "inobserv",
            ctx.exception.mensagem,
            "a destruicao parou, mas nao por inobservabilidade do task root",
        )


class OcupanteEMedicaoTests(Base):
    """Rodada 12: regressao de semantica e relatorio que subcontava.

    Estes sao ORACULOS COMPORTAMENTAIS de comando, que era a defesa que
    faltava: o gate sintatico nunca veria nenhuma das duas coisas.
    """

    def test_worktree_symlink_pendente_e_recusada_e_nao_provisionada(self) -> None:
        """A regressao que a troca de predicado introduziu.

        `exists()` segue symlink: link pendente contava como AUSENTE, caia na
        criacao e morria em `sem_symlink_em_componentes`. `estado_do_caminho`
        usa `lstat`: link pendente virou PRESENTE e PULOU o ramo, e o comando
        respondia PROVISIONED com um symlink quebrado no lugar da worktree.
        """
        dados = self.provisionar()
        worktree = Path(dados["task_root"]) / "worktree"
        shutil.rmtree(worktree)
        worktree.symlink_to(self.tmp / "alvo-que-nao-existe")

        codigo, r = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertNotEqual(codigo, 0, f"symlink pendente aceito como worktree: {r}")
        self.assertEqual(r.get("status"), "DENIED", r)

    def test_worktree_ocupada_por_arquivo_e_recusada(self) -> None:
        dados = self.provisionar()
        worktree = Path(dados["task_root"]) / "worktree"
        shutil.rmtree(worktree)
        worktree.write_text("nao sou diretorio\n", encoding="utf-8")

        codigo, r = self.rodar("provision", "--branch", "b2", "--base", "main")
        self.assertNotEqual(codigo, 0, f"arquivo aceito como worktree: {r}")
        self.assertEqual(r.get("status"), "DENIED", r)

    def test_arquivo_ilegivel_e_contado_como_ilegivel_e_nao_sumido(self) -> None:
        """Subcontar em silencio e afirmar o que nao se observou.

        O relatorio publica `reclaimed_bytes` como evidencia do que foi
        removido. Um `except OSError: continue` fazia o arquivo sair da conta
        com a mesma cara de um diretorio menor.
        """
        alvo = Path(self.tmp) / "arvore"
        (alvo / "sub").mkdir(parents=True)
        (alvo / "sub" / "grande.bin").write_bytes(b"x" * 4096)

        b, f, ilegiveis = fa.medir(alvo)
        self.assertEqual((b, f, ilegiveis), (4096, 1, 0))

        real_lstat = os.lstat

        def lstat_cego(caminho, *a, **k):
            if str(caminho).endswith("grande.bin"):
                raise PermissionError(13, "Permission denied")
            return real_lstat(caminho, *a, **k)

        os.lstat = lstat_cego
        try:
            b2, f2, ilegiveis2 = fa.medir(alvo)
        finally:
            os.lstat = real_lstat

        self.assertEqual(ilegiveis2, 1, "arquivo ilegivel sumiu da medicao sem deixar rastro")
        self.assertEqual(b2, 0)

    def test_task_root_inobservavel_impede_provisionar(self) -> None:
        """Oraculo de comando que a rodada 12 apontou faltar em cmd_provision."""
        dados = self.provisionar()
        task_root = Path(dados["task_root"])
        real_lstat = Path.lstat

        def lstat_cego(self, *a, **k):
            if str(self) == str(task_root):
                raise PermissionError(13, "Permission denied")
            return real_lstat(self, *a, **k)

        Path.lstat = lstat_cego
        try:
            codigo, r = self.rodar("provision", "--branch", "b3", "--base", "main")
        finally:
            Path.lstat = real_lstat
        self.assertNotEqual(codigo, 0, f"task root inobservavel foi provisionado: {r}")

    def test_slot_inobservavel_reprova_a_verificacao(self) -> None:
        """Oraculo de comando que a rodada 12 apontou faltar em cmd_verify."""
        dados = self.provisionar()
        task_root = Path(dados["task_root"])
        real_lstat = Path.lstat

        def lstat_cego(self, *a, **k):
            if str(self) == str(task_root):
                raise PermissionError(13, "Permission denied")
            return real_lstat(self, *a, **k)

        Path.lstat = lstat_cego
        try:
            codigo, r = self.rodar("verify")
        finally:
            Path.lstat = real_lstat
        self.assertNotEqual(codigo, 0, f"slot inobservavel passou na verificacao: {r}")


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
