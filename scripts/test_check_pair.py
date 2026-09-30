"""Regression tests for pair selection, command parity, and truthful evidence."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("check_pair", Path(__file__).with_name("check-pair.py"))
pair = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pair)
SHA = "a" * 40


class PairTests(unittest.TestCase):
    def test_pin_rejects_other_repository_short_or_mismatched_sha(self):
        good = {"worldline": {"repository": "ikzerok/worldline", "sha": SHA}}
        pair.validate_pair(good, SHA)
        for value, actual in [({}, SHA), ([], SHA), ({"worldline": None}, SHA),
                              ({"worldline": {"repository": "ikzerok/worldline", "sha": 123}}, SHA),
                              (good, "b" * 40),
                              ({"worldline": {"repository": "other/worldline", "sha": SHA}}, SHA),
                              ({"worldline": {"repository": "ikzerok/worldline", "sha": "main"}}, "main")]:
            with self.assertRaises(pair.CheckFailure):
                pair.validate_pair(value, actual)

    def test_steps_match_windows_checker_and_preserve_locked_strict_targets(self):
        steps = pair.checks(Path("/tmp/a path/worldedit"), Path("/tmp/a path/worldline"))
        windows = Path(__file__).with_name("check-pair.ps1").read_text()
        import re
        self.assertEqual([name for name, _ in steps], re.findall(r"Invoke-Recorded '([^']+)'", windows))
        for name, command in steps:
            if any(name.endswith(suffix) for suffix in ["-test", "-clippy", "-build"]):
                self.assertIn("--locked", command)
            if name.endswith("-clippy"):
                self.assertEqual(command[-3:], ["--", "-D", "warnings"])
            if "wasm" in name:
                self.assertIn("wasm32-unknown-unknown", command)

    def test_recorded_captures_failure_and_missing_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(pair.CheckFailure), contextlib.redirect_stdout(io.StringIO()):
                pair.recorded("bad", [sys.executable, "-c", "print('evidence'); raise SystemExit(3)"], root, root)
            self.assertIn("evidence\nexit code: 3", (root / "bad.log").read_text())
            with self.assertRaises(pair.CheckFailure):
                pair.recorded("missing", [str(root / "absent-command")], root, root)
            self.assertIn("not started:", (root / "missing.log").read_text())

    def test_each_run_inherits_environment_but_isolates_all_temp_variables(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            with patch.dict(pair.os.environ, {"PAIR_TEST_SENTINEL": "kept"}):
                first, environment = pair.isolated_environment()
                second, other = pair.isolated_environment()
            self.assertNotEqual(first, second)
            self.assertTrue(first.is_dir() and second.is_dir())
            self.assertEqual(environment["PAIR_TEST_SENTINEL"], "kept")
            for key in ("TMPDIR", "TMP", "TEMP"):
                self.assertEqual(environment[key], str(first))
                self.assertEqual(other[key], str(second))
            command = [sys.executable, "-c", "import os,tempfile; print(tempfile.gettempdir()); print(os.environ['PAIR_TEST_SENTINEL'])"]
            with contextlib.redirect_stdout(io.StringIO()):
                pair.recorded("temp", command, evidence, evidence, environment)
            log = (evidence / "temp.log").read_text()
            self.assertIn(str(first) + "\nkept\nexit code: 0", log)
            self.assertTrue(first.exists(), "证据临时根不能由检查器自动删除")
            first.rmdir()
            second.rmdir()

    def exercise_run(self, fail=None, actual=SHA, missing=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            editor, language, evidence = [root / part for part in ["worldedit", "worldline", "evidence"]]
            for path in [editor, language, evidence]:
                path.mkdir()
            (editor / "compatibility.json").write_text(json.dumps({"worldline": {"repository": "ikzerok/worldline", "sha": SHA}}))
            for repo in [editor, language]:
                (repo / "Cargo.lock").write_text("lock")
            invoked = []
            def record(name, *_):
                invoked.append(name)
                if name == fail:
                    if missing:
                        raise pair.CheckNotStarted("missing tool")
                    raise pair.CheckFailure("intentional failure")
            def git(_, *args):
                return actual if args[0] == "rev-parse" else " M example.rs"
            with patch.object(pair, "git", git), patch.object(pair, "recorded", record), contextlib.redirect_stderr(io.StringIO()):
                code = pair.run(editor, evidence)
            summary = json.loads((evidence / "summary.json").read_text())
            environment_path = evidence / "environment.json"
            environment = json.loads(environment_path.read_text()) if environment_path.exists() else None
            return code, summary, environment, invoked

    def test_failed_step_does_not_claim_later_checks_ran(self):
        code, summary, _, invoked = self.exercise_run(fail="worldline-test")
        self.assertEqual(code, 1)
        statuses = {step["name"]: step["status"] for step in summary["checks"]}
        self.assertEqual(statuses["worldline-fmt"], "passed")
        self.assertEqual(statuses["worldline-test"], "failed")
        self.assertEqual(statuses["worldline-clippy"], "not_run")
        self.assertEqual(invoked[-1], "worldline-test")

    def test_missing_tool_is_not_run_in_summary(self):
        code, summary, _, invoked = self.exercise_run(fail="rustc", missing=True)
        self.assertEqual(code, 1)
        self.assertEqual(invoked, ["rustc"])
        self.assertTrue(all(step["status"] == "not_run" for step in summary["checks"]))
        self.assertEqual(summary["checks"][0]["reason"], "missing tool")

    def test_pair_mismatch_runs_no_commands(self):
        code, summary, environment, invoked = self.exercise_run(actual="b" * 40)
        self.assertEqual(code, 1)
        self.assertIsNone(environment)
        self.assertEqual(invoked, [])
        self.assertTrue(all(step["status"] == "not_run" for step in summary["checks"]))

    def test_success_records_dirty_tree_and_never_claims_gui_acceptance(self):
        code, summary, environment, invoked = self.exercise_run()
        self.assertEqual(code, 0)
        self.assertEqual(summary["status"], "passed")
        self.assertEqual(len(invoked), 14)
        self.assertIn("not performed", environment["gui_acceptance"])
        self.assertEqual(environment["worldedit_working_tree"], " M example.rs")


if __name__ == "__main__":
    unittest.main()
