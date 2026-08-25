from __future__ import annotations

import copy
import fcntl
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path


BOOK_SCRIPT = Path(__file__).resolve().parents[1] / "book.py"
SPEC = importlib.util.spec_from_file_location("book_v0", BOOK_SCRIPT)
assert SPEC and SPEC.loader
book_v0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(book_v0)


def valid_draft(case_id: str = "B-7K4M2Q9R6T3V") -> dict:
    return {
        "id": case_id,
        "schema_version": 1,
        "title": "Native runtime test can consume a stale archive",
        "cues": ["native-test", "pinker_rt", "stale-artifact"],
        "scope": {
            "components": ["runtime", "backend-s"],
            "conditions": ["native test links libpinker_rt.a"],
        },
        "observed_at": None,
        "environment": {"repository": "LyannaValerie/pinker-v0"},
        "problem": {
            "class": "OBSERVED",
            "text": "A native test continued to consume an old libpinker_rt.a after runtime source changed.",
        },
        "discriminating_probe": {
            "action": "Rebuild pinker_rt explicitly, then rerun the same native test.",
            "observable": "The archive changes and the test reflects the runtime mutation.",
        },
        "observed_result": {
            "class": "OBSERVED",
            "text": "An explicit runtime build replaced the stale archive used by the test.",
        },
        "guidance": {
            "class": "ASSERTED",
            "text": "Treat a green sensitivity run as inconclusive until the mutated runtime artifact is rebuilt.",
        },
        "contraindications": [
            "Do not generalize to tests that do not link pinker_rt.",
            "A fresh build already rules out this stale-artifact hypothesis.",
        ],
        "evidence": [
            {
                "class": "OBSERVED",
                "description": "Operational memory records the stale archive and explicit rebuild.",
                "source": "file:/pinker/msg/campanhas/maturacao-adulta/d13-resource-theory-adult.md",
            }
        ],
        "references": [
            "trama:backend-s.pipeline.nativo-runtime",
            "file:/pinker/msg/campanhas/maturacao-adulta/d13-resource-theory-adult.md",
            "git:833c35fac288abed100cc1bd69064ba418defa4b",
        ],
        "status": "candidate",
        "challenges": [],
        "revision": {
            "updated_at": "2026-08-24T23:20:00Z",
            "updated_by": "fixture",
            "reason": "initial test case",
        },
    }


class BookCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "book"
        self.inputs = Path(self.temp.name) / "inputs"
        self.inputs.mkdir()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_json(self, name: str, value: object) -> Path:
        path = self.inputs / name
        path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
        return path

    def cli(self, *arguments: str) -> tuple[int, dict]:
        completed = subprocess.run(
            [sys.executable, str(BOOK_SCRIPT), "--book", str(self.root), *arguments],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.stderr, "")
        return completed.returncode, json.loads(completed.stdout)

    def cli_command(self, *arguments: str) -> list[str]:
        return [sys.executable, str(BOOK_SCRIPT), "--book", str(self.root), *arguments]

    def add(self, draft: dict | None = None, *, allow_similar: bool = False) -> dict:
        input_path = self.write_json("case.json", draft or valid_draft())
        arguments = ["add-case", str(input_path)]
        if allow_similar:
            arguments.append("--allow-similar")
        code, result = self.cli(*arguments)
        self.assertEqual((code, result.get("ok")), (0, True), result)
        return result

    def test_empty_book_search_is_normal(self) -> None:
        code, result = self.cli("search", "stderr")
        self.assertEqual(code, 0)
        self.assertEqual(result["count"], 0)
        self.assertEqual(result["results"], [])

    def test_add_show_and_verify_valid_case(self) -> None:
        added = self.add()
        code, shown = self.cli("show", "B-7K4M2Q9R6T3V")
        self.assertEqual(code, 0)
        self.assertEqual(shown["content_trust"], "UNTRUSTED_DATA")
        self.assertEqual(shown["case"]["revision"]["hash"], added["revision"])
        self.assertIn("scope", shown["case"])
        self.assertIn("discriminating_probe", shown["case"])
        self.assertIn("evidence", shown["case"])
        self.assertIn("contraindications", shown["case"])
        code, verified = self.cli("verify")
        self.assertEqual((code, verified["files"], verified["valid_cases"]), (0, 1, 1))
        self.assertEqual(verified["errors"], [])

    def test_search_is_deterministic_and_compact(self) -> None:
        self.add()
        first = self.cli("search", "runtime stale-artifact")
        second = self.cli("search", "runtime stale-artifact")
        self.assertEqual(first, second)
        result = first[1]["results"][0]
        self.assertEqual(result["id"], "B-7K4M2Q9R6T3V")
        self.assertNotIn("problem", result)
        self.assertNotIn("guidance", result)

    def test_duplicate_id_is_rejected(self) -> None:
        self.add()
        code, result = self.cli("add-case", str(self.write_json("duplicate-id.json", valid_draft())))
        self.assertEqual((code, result["error"]), (2, "CASE_ID_DUPLICATE"))

    def test_exact_duplicate_content_is_rejected(self) -> None:
        self.add()
        duplicate = valid_draft("B-3N8C5W2H9J6X")
        code, result = self.cli("add-case", str(self.write_json("exact.json", duplicate)))
        self.assertEqual((code, result["error"]), (2, "CASE_EXACT_DUPLICATE"))

    def test_schema_invalid_and_future_schema_fail_closed(self) -> None:
        invalid = valid_draft()
        invalid.pop("scope")
        code, result = self.cli("add-case", str(self.write_json("invalid.json", invalid)))
        self.assertEqual((code, result["error"]), (2, "SCHEMA_INVALID"))
        future = valid_draft()
        future["schema_version"] = 2
        code, result = self.cli("add-case", str(self.write_json("future.json", future)))
        self.assertEqual((code, result["error"]), (2, "SCHEMA_FUTURE_UNSUPPORTED"))
        boolean = valid_draft()
        boolean["schema_version"] = True
        code, result = self.cli("add-case", str(self.write_json("boolean.json", boolean)))
        self.assertEqual((code, result["error"]), (2, "SCHEMA_UNSUPPORTED"))

    def test_verify_reports_invalid_json_and_future_schema(self) -> None:
        (self.root / "cases").mkdir(parents=True)
        (self.root / "cases" / "broken.json").write_text("{", encoding="utf-8")
        future = book_v0.prepare_new_case(valid_draft())
        future["schema_version"] = 2
        future["revision"]["hash"] = book_v0.revision_hash(future)
        (self.root / "cases" / f"{future['id']}.json").write_text(json.dumps(future), encoding="utf-8")
        code, result = self.cli("verify")
        self.assertEqual(code, 1)
        self.assertEqual(
            {error["code"] for error in result["errors"]},
            {"PARSE_FAILED", "SCHEMA_FUTURE_UNSUPPORTED"},
        )

    def test_verify_rejects_duplicate_json_keys(self) -> None:
        (self.root / "cases").mkdir(parents=True)
        (self.root / "cases" / "ambiguous.json").write_text(
            '{"id":"B-7K4M2Q9R6T3V","id":"B-3N8C5W2H9J6X"}', encoding="utf-8"
        )
        code, result = self.cli("verify")
        self.assertEqual(code, 1)
        self.assertEqual(result["errors"][0]["code"], "JSON_DUPLICATE_KEY")

    def test_verify_detects_duplicate_ids(self) -> None:
        self.add()
        source = self.root / "cases" / "B-7K4M2Q9R6T3V.json"
        shutil.copyfile(source, self.root / "cases" / "copy.json")
        code, result = self.cli("verify")
        self.assertEqual(code, 1)
        self.assertIn("CASE_ID_DUPLICATE", {error["code"] for error in result["errors"]})

    def test_references_valid_and_invalid(self) -> None:
        self.add()
        invalid = book_v0.prepare_new_case(valid_draft("B-0A1B2C3D4E5F"))
        invalid["references"] = ["trama:"]
        invalid["revision"]["hash"] = book_v0.revision_hash(invalid)
        (self.root / "cases" / f"{invalid['id']}.json").write_text(json.dumps(invalid), encoding="utf-8")
        code, result = self.cli("verify")
        self.assertEqual(code, 1)
        self.assertIn("REFERENCE_INVALID", {error["code"] for error in result["errors"]})

    def test_revise_requires_current_revision(self) -> None:
        added = self.add()
        patch = {"guidance": {"class": "ASSERTED", "text": "Rebuild the runtime artifact before interpreting the result."}}
        patch_path = self.write_json("patch.json", patch)
        common = [
            "--patch", str(patch_path),
            "--updated-at", "2026-08-24T23:21:00Z",
            "--updated-by", "reviewer",
            "--reason", "tighten guidance",
        ]
        code, revised = self.cli("revise", "B-7K4M2Q9R6T3V", "--if-revision", added["revision"], *common)
        self.assertEqual(code, 0)
        self.assertEqual(revised["parent_revision"], added["revision"])
        code, conflict = self.cli("revise", "B-7K4M2Q9R6T3V", "--if-revision", added["revision"], *common)
        self.assertEqual((code, conflict["error"]), (2, "REVISION_CONFLICT"))
        self.assertEqual(conflict["details"]["current"], revised["revision"])

    def test_concurrent_revise_is_an_atomic_compare_and_swap(self) -> None:
        added = self.add()
        first_patch = self.write_json("first-patch.json", {"title": "first winner"})
        second_patch = self.write_json("second-patch.json", {"title": "second winner"})
        common = [
            "--if-revision", added["revision"],
            "--updated-at", "2026-08-24T23:21:00Z",
            "--updated-by", "race-fixture",
            "--reason", "concurrent CAS probe",
        ]
        directory_fd = os.open(self.root / "cases", os.O_RDONLY | os.O_DIRECTORY)
        fcntl.flock(directory_fd, fcntl.LOCK_EX)
        processes = [
            subprocess.Popen(
                self.cli_command("revise", "B-7K4M2Q9R6T3V", *common, "--patch", str(patch)),
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            for patch in (first_patch, second_patch)
        ]
        time.sleep(0.1)
        fcntl.flock(directory_fd, fcntl.LOCK_UN)
        os.close(directory_fd)
        results = []
        for process in processes:
            stdout, stderr = process.communicate(timeout=10)
            self.assertEqual(stderr, "")
            results.append((process.returncode, json.loads(stdout)))
        self.assertEqual(sorted(code for code, _ in results), [0, 2])
        self.assertEqual(
            [result["error"] for code, result in results if code == 2], ["REVISION_CONFLICT"]
        )

    def test_concurrent_add_prevents_exact_duplicate(self) -> None:
        (self.root / "cases").mkdir(parents=True)
        first = self.write_json("first-case.json", valid_draft("B-7K4M2Q9R6T3V"))
        second = self.write_json("second-case.json", valid_draft("B-3N8C5W2H9J6X"))
        directory_fd = os.open(self.root / "cases", os.O_RDONLY | os.O_DIRECTORY)
        fcntl.flock(directory_fd, fcntl.LOCK_EX)
        processes = [
            subprocess.Popen(
                self.cli_command("add-case", str(draft)),
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            for draft in (first, second)
        ]
        time.sleep(0.1)
        fcntl.flock(directory_fd, fcntl.LOCK_UN)
        os.close(directory_fd)
        results = []
        for process in processes:
            stdout, stderr = process.communicate(timeout=10)
            self.assertEqual(stderr, "")
            results.append((process.returncode, json.loads(stdout)))
        self.assertEqual(sorted(code for code, _ in results), [0, 2])
        self.assertEqual(
            [result["error"] for code, result in results if code == 2],
            ["CASE_EXACT_DUPLICATE"],
        )
        self.assertEqual(len(list((self.root / "cases").glob("*.json"))), 1)

    def test_challenge_preserves_contrary_evidence_and_revision(self) -> None:
        added = self.add()
        challenge = {
            "id": "C-9Q7W5E3R2T",
            "observed_at": "2026-08-24T23:22:00Z",
            "reported_by": "reviewer",
            "statement": {"class": "ASSERTED", "text": "A clean build reproduced the symptom, so stale runtime is not sufficient."},
            "evidence": [
                {
                    "class": "VALIDATED",
                    "description": "The symptom persisted after a clean rebuild.",
                    "source": "git:833c35fac288abed100cc1bd69064ba418defa4b",
                    "oracle": "A clean rebuild followed by the same failing native test.",
                }
            ],
            "references": ["file:tests/native_runtime_case.pink"],
        }
        code, result = self.cli(
            "challenge", "B-7K4M2Q9R6T3V",
            "--if-revision", added["revision"],
            "--challenge", str(self.write_json("challenge.json", challenge)),
            "--updated-at", "2026-08-24T23:23:00Z",
            "--updated-by", "reviewer",
            "--reason", "record counter-evidence",
        )
        self.assertEqual(code, 0)
        self.assertEqual(result["status"], "challenged")
        code, shown = self.cli("show", "B-7K4M2Q9R6T3V")
        self.assertEqual(code, 0)
        self.assertEqual(shown["case"]["challenges"][0]["evidence"][0]["class"], "VALIDATED")
        self.assertEqual(shown["case"]["revision"]["parent_hash"], added["revision"])

    def test_near_match_is_candidate_not_applicability_claim(self) -> None:
        self.add()
        near = valid_draft("B-3N8C5W2H9J6X")
        near["title"] = "Stale archive during a hosted-only test"
        near["scope"] = {"components": ["interpreter"], "conditions": ["hosted-only test does not link native runtime"]}
        near["problem"]["text"] = "A hosted-only test reported an old artifact symptom."
        near["contraindications"] = ["Does not apply to native tests linking libpinker_rt.a."]
        code, report = self.cli("add-case", str(self.write_json("near.json", near)))
        self.assertEqual((code, report["error"]), (2, "CASE_SIMILAR_CANDIDATES"))
        self.assertEqual(report["details"]["candidates"][0]["id"], "B-7K4M2Q9R6T3V")
        self.add(near, allow_similar=True)
        code, search = self.cli("search", "stale-artifact")
        self.assertEqual(code, 0)
        self.assertEqual(search["count"], 2)
        self.assertNotIn("applicable", json.dumps(search).casefold())
        self.assertNotEqual(search["results"][0]["scope"], search["results"][1]["scope"])

    def test_validated_claim_requires_an_explicit_oracle(self) -> None:
        draft = valid_draft()
        draft["guidance"] = {"class": "VALIDATED", "text": "A claim without an oracle."}
        code, result = self.cli("add-case", str(self.write_json("no-oracle.json", draft)))
        self.assertEqual((code, result["error"]), (2, "SCHEMA_INVALID"))

    def test_secret_like_content_is_not_persisted(self) -> None:
        draft = valid_draft()
        draft["guidance"]["text"] = "credential password=hunter2"
        code, result = self.cli("add-case", str(self.write_json("secret.json", draft)))
        self.assertEqual((code, result["error"]), (2, "SENSITIVE_CONTENT"))
        self.assertFalse((self.root / "cases").exists())

    def test_challenged_status_requires_challenge_evidence(self) -> None:
        draft = valid_draft()
        draft["status"] = "challenged"
        code, result = self.cli("add-case", str(self.write_json("empty-challenge.json", draft)))
        self.assertEqual((code, result["error"]), (2, "SCHEMA_INVALID"))

    def test_search_scope_is_bounded_and_marks_truncation(self) -> None:
        draft = valid_draft()
        draft["scope"] = {
            "components": [("component-%02d-" % index) + "x" * 490 for index in range(32)],
            "conditions": [("condition-%02d-" % index) + "y" * 490 for index in range(32)],
        }
        self.add(draft)
        code, result = self.cli("search", "native-test")
        self.assertEqual(code, 0)
        self.assertTrue(result["results"][0]["scope"]["truncated"])
        self.assertLess(len(json.dumps(result)), 5000)

    def test_search_query_and_matched_cues_are_bounded(self) -> None:
        self.add()
        code, result = self.cli("search", "x" * 513)
        self.assertEqual((code, result["error"]), (2, "CONTENT_LIMIT"))
        code, result = self.cli("search", "native-test")
        self.assertEqual(code, 0)
        self.assertFalse(result["results"][0]["matched_cues_truncated"])

    def test_unknown_scope_is_explicit_not_universal(self) -> None:
        draft = valid_draft()
        draft["scope"] = None
        self.add(draft)
        code, shown = self.cli("show", "B-7K4M2Q9R6T3V")
        self.assertEqual(code, 0)
        self.assertIsNone(shown["case"]["scope"])


if __name__ == "__main__":
    unittest.main()
