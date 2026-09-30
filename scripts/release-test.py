"""发行协议离线测试：不读凭据、不联网、不创建 tag/release。"""
import base64
import copy
import hashlib
import importlib.util
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
E, C = "a" * 40, "b" * 40


class GateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.event_path = Path(self.temp.name) / "event.json"
        self.event = {"ref": "release/v0.3.1", "ref_type": "branch", "repository": {"full_name": release.EDITOR}}
        self.save_event()
        self.env = patch.dict(os.environ, {"GITHUB_EVENT_NAME": "create", "GITHUB_REPOSITORY": release.EDITOR,
            "GITHUB_SHA": E, "GITHUB_REF": "refs/heads/release/v0.3.1", "GITHUB_EVENT_PATH": str(self.event_path)})
        self.env.start()
        self.addCleanup(self.env.stop)
        self.version = "0.3.1"
        self.editor_main, self.core_main = E, C
        self.issue_list = [{"number": 1, "pull_request": {}}]
        self.status, self.conclusion = "completed", "success"
        self.api_mock = patch.object(release, "api", side_effect=self.api).start()
        self.addCleanup(patch.stopall)
        def mock_pages(path, collection=None):
            if collection == "workflow_runs": return self.api_mock(path)["workflow_runs"]
            return self.issue_list
        patch.object(release, "pages", side_effect=mock_pages).start()
        self.absent = patch.object(release, "absent").start()

    def save_event(self):
        self.event_path.write_text(json.dumps(self.event), encoding="utf-8")

    def api(self, path, method="GET", payload=None):
        self.assertEqual(method, "GET", "gate 必须只读")
        if path.endswith("/git/ref/heads/main"):
            return {"object": {"sha": self.core_main if release.CORE in path else self.editor_main}}
        if "/git/ref/heads/release/" in path:
            return {"object": {"sha": E}}
        if "/git/ref/tags/" in path:
            return {"object": {"sha": E, "type": "commit"}}
        if "/contents/" in path:
            if "compatibility.json" in path:
                data = json.dumps({"worldline": {"repository": release.CORE, "sha": C}})
            elif release.CORE in path:
                data = '[workspace.package]\nversion = "0.3.0"\n'
            else:
                data = '[package]\nversion = "' + self.version + '"\n'
            return {"encoding": "base64", "content": base64.b64encode(data.encode()).decode()}
        if "/actions/workflows/ci.yml/runs?" in path:
            core = release.CORE in path
            return {"workflow_runs": [{"head_sha": C if core else E, "head_branch": "main", "event": "push",
                "head_repository": {"full_name": release.CORE if core else release.EDITOR},
                "run_number": 4, "run_attempt": 1, "id": 99, "html_url": "https://github.com/example/actions/runs/99",
                "status": self.status, "conclusion": self.conclusion}]}
        raise AssertionError(path)

    def test_valid_pair_and_pr_is_not_issue(self):
        result = release.gate()
        self.assertEqual(result["worldedit"]["sha"], E)
        self.assertEqual(result["worldline"]["version"], "0.3.0")
        self.assertEqual(self.absent.call_count, 2)

    def test_wrong_repository(self):
        with patch.dict(os.environ, {"GITHUB_REPOSITORY": "attacker/worldedit"}):
            with self.assertRaises(RuntimeError): release.gate()

    def test_wrong_event_repository(self):
        self.event["repository"]["full_name"] = "attacker/worldedit"
        self.save_event()
        with self.assertRaises(RuntimeError): release.gate()

    def test_non_create_event(self):
        for event in ["push", "pull_request", "workflow_dispatch"]:
            with self.subTest(event=event), patch.dict(os.environ, {"GITHUB_EVENT_NAME": event}):
                with self.assertRaises(RuntimeError): release.gate()

    def test_tag_event(self):
        self.event["ref_type"] = "tag"
        self.save_event()
        with self.assertRaises(RuntimeError): release.gate()

    def test_branch_injection_or_noncanonical(self):
        for branch in ["release/v0.3.1;echo bad", "release/v0.3.1\n", "release/v01.3.1", "release/v0.3.1-rc1",
                       "main", "release/v0.3.1/../evil", "release/v0.3.$(id)"]:
            self.event["ref"] = branch
            self.save_event()
            with self.subTest(branch=branch), self.assertRaises(RuntimeError): release.gate()

    def test_cargo_version_mismatch(self):
        self.version = "0.3.0"
        with self.assertRaises(RuntimeError): release.gate()

    def test_editor_main_moved(self):
        self.editor_main = "c" * 40
        with self.assertRaises(RuntimeError): release.gate()

    def test_core_main_moved(self):
        self.core_main = "c" * 40
        with self.assertRaises(RuntimeError): release.gate()

    def test_open_issue(self):
        self.issue_list.append({"number": 2})
        with self.assertRaises(RuntimeError): release.gate()

    def test_ci_failed_or_unfinished(self):
        for status, conclusion in [("completed", "failure"), ("in_progress", None), ("queued", None), ("completed", "cancelled")]:
            self.status, self.conclusion = status, conclusion
            with self.subTest(status=status, conclusion=conclusion), self.assertRaises(RuntimeError): release.gate()

    def test_latest_ci_failure_beats_old_success(self):
        original = self.api
        def api(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if "workflow_runs" in result:
                newer = copy.deepcopy(result["workflow_runs"][0])
                newer.update(run_number=5, conclusion="failure")
                result["workflow_runs"].append(newer)
            return result
        self.api_mock.side_effect = api
        with self.assertRaises(RuntimeError): release.gate()

    def test_wrong_ci_sha(self):
        original = self.api
        def api(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if "workflow_runs" in result: result["workflow_runs"][0]["head_sha"] = "c" * 40
            return result
        self.api_mock.side_effect = api
        with self.assertRaises(RuntimeError): release.gate()

    def test_existing_release_or_tag_fails(self):
        self.absent.side_effect = RuntimeError("已存在")
        with self.assertRaises(RuntimeError): release.gate()

    def test_main_changes_during_gate(self):
        original = self.api
        def api(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if "/actions/workflows/" in path: self.editor_main = "c" * 40
            return result
        self.api_mock.side_effect = api
        with self.assertRaises(RuntimeError): release.gate()


class AssetTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.pair = {"schema_version": 1, "tag": "v0.3.1", "worldedit": {"repository": release.EDITOR, "sha": E, "version": "0.3.1"},
                     "worldline": {"repository": release.CORE, "sha": C, "version": "0.3.0"}}
        self.files = {"worldedit-windows-x64.zip": {f"windows/{n}.exe": b"test" for n in ["worldedit", "wl", "wl-agent"]},
                      "worldedit-web.zip": {"web/index.html": b"test", "web/app.js": b"test", "web/app.wasm": b"test"},
                      "worldedit-source.zip": {"worldedit/Cargo.toml": b'[package]\nversion="0.3.1"',
                        "worldedit/compatibility.json": json.dumps({"worldline": {"repository": release.CORE, "sha": C}}).encode()},
                      "worldline-source.zip": {"worldline/Cargo.toml": b'[workspace.package]\nversion="0.3.0"'}}
        self.write()

    def write(self):
        for name, files in self.files.items():
            with zipfile.ZipFile(self.root / name, "w") as archive:
                for path, data in files.items(): archive.writestr(path, data)
                if name.endswith("-source.zip"):
                    archive.comment = self.pair[name.removesuffix("-source.zip")]["sha"].encode()
        (self.root / "release-pair.json").write_text(json.dumps(self.pair), encoding="utf-8")
        self.hashes()

    def hashes(self):
        (self.root / "SHA256SUMS.txt").write_text("".join(
            hashlib.sha256((self.root / name).read_bytes()).hexdigest() + "  " + name + "\n"
            for name in sorted(release.ASSETS - {"SHA256SUMS.txt"})), encoding="utf-8")

    def test_valid_assets(self): release.verify_assets(self.root, self.pair)

    def test_hash_mismatch(self):
        (self.root / "worldedit-web.zip").write_bytes(b"tampered")
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_manifest_wrong_pair(self):
        other = copy.deepcopy(self.pair)
        other["worldline"]["sha"] = "c" * 40
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, other)

    def test_extra_asset(self):
        (self.root / "private.png").write_bytes(b"private")
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_missing_asset(self):
        (self.root / "worldline-source.zip").unlink()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_zip_traversal(self):
        self.files["worldedit-web.zip"]["../bad"] = b"test"
        self.write()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_windows_and_web_cannot_contain_other_roots(self):
        for name in ["worldedit-windows-x64.zip", "worldedit-web.zip"]:
            with self.subTest(name=name):
                self.files[name]["other-root.txt"] = b"extra"
                self.write()
                with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)
                del self.files[name]["other-root.txt"]

    def test_missing_binary(self):
        del self.files["worldedit-windows-x64.zip"]["windows/wl.exe"]
        self.write()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_source_version_mismatch(self):
        self.files["worldedit-source.zip"]["worldedit/Cargo.toml"] = b'[package]\nversion="0.3.0"'
        self.write()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_duplicate_hash_entry(self):
        file = self.root / "SHA256SUMS.txt"
        file.write_text(file.read_text() * 2)
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)


    def test_source_commit_marker_mismatch(self):
        with zipfile.ZipFile(self.root / "worldline-source.zip", "a") as archive:
            archive.comment = b"c" * 40
        self.hashes()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_source_compatibility_mismatch(self):
        self.files["worldedit-source.zip"]["worldedit/compatibility.json"] = b'{"worldline": {"sha": "wrong"}}'
        self.write()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def publication(self, fail_final_gate=False, tamper_download=False, existing_draft=False, duplicate_draft=False):
        assets = [{"id": index, "name": name, "state": "uploaded", "size": (self.root / name).stat().st_size,
                   "digest": "sha256:" + hashlib.sha256((self.root / name).read_bytes()).hexdigest()}
                  for index, name in enumerate(sorted(release.ASSETS))]
        state = {"id": 12, "draft": True, "tag_name": self.pair["tag"], "target_commitish": E,
                 "assets": assets, "html_url": "https://example.invalid/mock-release"}
        calls = []
        created = False
        def api(path, method="GET", payload=None):
            nonlocal created
            calls.append((path, method, payload))
            if method == "POST" and path.endswith("/releases"): created = True
            if method == "PATCH": state["draft"] = False
            return copy.deepcopy(state)
        def gh(*args, **kwargs):
            if args[:2] == ("release", "download"):
                destination = Path(args[args.index("--dir") + 1])
                for name in release.ASSETS: shutil.copyfile(self.root / name, destination / name)
                if tamper_download: (destination / "worldedit-web.zip").write_bytes(b"bad")
            return ""
        def page_list(path):
            if "/releases?" in path:
                matches = [copy.deepcopy(state)] if created or existing_draft else []
                if duplicate_draft and created:
                    other = copy.deepcopy(state)
                    other["id"] = 999
                    matches.append(other)
                return matches
            return assets
        results = [self.pair, self.pair, RuntimeError("main moved") if fail_final_gate else self.pair]
        with patch.object(release, "gate", side_effect=results), patch.object(release, "api", side_effect=api), \
             patch.object(release, "pages", side_effect=page_list), patch.object(release, "gh", side_effect=gh):
            if fail_final_gate or tamper_download or existing_draft or duplicate_draft:
                with self.assertRaises(RuntimeError): release.publish(self.root)
                self.assertFalse(any(method == "PATCH" for _, method, _ in calls))
                if existing_draft: self.assertFalse(calls, "已有 draft 时不得先创建 tag")
            else:
                release.publish(self.root)
                self.assertEqual([method for _, method, _ in calls].count("PATCH"), 1)
                self.assertEqual(calls[0][2], {"ref": "refs/tags/v0.3.1", "sha": E})
                self.assertTrue(calls[1][2]["draft"])

    def test_draft_upload_download_then_publish(self): self.publication()
    def test_final_gate_failure_leaves_draft(self): self.publication(fail_final_gate=True)
    def test_remote_asset_tamper_leaves_draft(self): self.publication(tamper_download=True)
    def test_existing_draft_without_tag_prevents_all_mutation(self): self.publication(existing_draft=True)
    def test_same_tag_second_draft_prevents_upload(self): self.publication(duplicate_draft=True)

    def test_private_paths_in_all_zip_kinds(self):
        attacks = [
            ("worldedit-source.zip", "worldedit/docs/qa/private.png", b"image"),
            ("worldedit-source.zip", "worldedit/.tooling/credential.json", b"{}"),
            ("worldedit-source.zip", "worldedit/.aws/config", b"fake"),
            ("worldedit-source.zip", "worldedit/.ssh/id_rsa", b"fake"),
            ("worldedit-source.zip", "worldedit/credentials.json", b"{}"),
            ("worldedit-windows-x64.zip", "windows/docs/qa/private.png", b"image"),
            ("worldedit-web.zip", "web/.env", b"FAKE=value"),
            ("worldline-source.zip", "worldline/docs/qa/private.mp4", b"video"),
            ("worldedit-source.zip", "worldedit/docs/qa/private.b64", b"encoded"),
            ("worldedit-source.zip", "worldedit/docs/qa/private.txt", b"iVBORw0KGgoAAAANSUhEUg"),
            ("worldedit-source.zip", "worldedit/docs/qa/private.txt", b"\x89PNG\r\n\x1a\n"),
        ]
        for name, path, data in attacks:
            with self.subTest(path=path):
                self.files[name][path] = data
                self.write()
                with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)
                del self.files[name][path]

    def test_archive_path_aliases(self):
        for path in ["web\\.env", "web/./file.txt", "web//file.txt", "web/file.txt.", "web/file.txt ",
                     "web/%2eenv", "web/INDEX.HTML", "web/cafe\u0301.txt"]:
            with self.subTest(path=path):
                self.files["worldedit-web.zip"][path] = b"test"
                self.write()
                with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)
                del self.files["worldedit-web.zip"][path]

    def test_symlink_rejected_in_final_asset(self):
        with zipfile.ZipFile(self.root / "worldedit-web.zip", "a") as archive:
            entry = zipfile.ZipInfo("web/link.txt")
            entry.create_system = 3
            entry.external_attr = 0o120777 << 16
            archive.writestr(entry, "outside.txt")
        self.hashes()
        with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_qa_worldline_fixture_text_allowed(self):
        self.files["worldedit-source.zip"]["worldedit/docs/qa/fixtures/replay-gates/world.wl"] = "# 公开 worldline 验收样例\n".encode("utf-8")
        self.write()
        release.verify_assets(self.root, self.pair)

    def test_qa_worldline_suffix_does_not_allow_media(self):
        path = "worldedit/docs/qa/fixtures/replay-gates/world.wl"
        for data in [b"\x89PNG\r\n\x1a\n", b"iVBORw0KGgoAAAANSUhEUg", b"data:image/png;base64,fixture", b"binary\0fixture"]:
            with self.subTest(data=data):
                self.files["worldedit-source.zip"][path] = data
                self.write()
                with self.assertRaises(RuntimeError): release.verify_assets(self.root, self.pair)

    def test_normal_public_icons_and_text_qa_remain_allowed(self):
        self.files["worldedit-web.zip"]["web/assets/icon.png"] = b"public icon fixture"
        self.files["worldedit-source.zip"]["worldedit/docs/qa/evidence.json"] = b'{"status":"passed"}'
        self.files["worldline-source.zip"]["worldline/docs/qa/test.log"] = b"test exited 0"
        self.write()
        release.verify_assets(self.root, self.pair)


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repository"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        self.git("config", "user.name", "Offline test")
        self.git("config", "user.email", "test@example.invalid")
        (self.repo / ".agent").mkdir()
        (self.repo / ".agent/README.md").write_text("public text")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        self.sha = self.git("rev-parse", "HEAD").strip()
        spec = importlib.util.spec_from_file_location("release_build", Path(__file__).with_name("release-build.py"))
        self.builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.builder)

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], text=True)

    def test_only_committed_snapshot_and_hidden_files(self):
        (self.repo / "private.png").write_bytes(b"not public")
        output = self.root / "source.zip"
        self.builder.snapshot(self.repo, "worldedit", self.sha, output)
        with zipfile.ZipFile(output) as archive:
            self.assertIn("worldedit/.agent/README.md", archive.namelist())
            self.assertNotIn("worldedit/private.png", archive.namelist())
            self.assertEqual(archive.comment.decode(), self.sha)

    def test_tracked_qa_screenshot_fails(self):
        (self.repo / "docs/qa").mkdir(parents=True)
        (self.repo / "docs/qa/private.png").write_bytes(b"private")
        self.git("add", ".")
        self.git("commit", "-qm", "private fixture")
        sha = self.git("rev-parse", "HEAD").strip()
        with self.assertRaises(RuntimeError):
            self.builder.snapshot(self.repo, "worldedit", sha, self.root / "source.zip")

    def test_wrong_checkout_sha_fails(self):
        with self.assertRaises(RuntimeError):
            self.builder.snapshot(self.repo, "worldedit", "c" * 40, self.root / "source.zip")


class AbsentTests(unittest.TestCase):
    def test_only_explicit_404_is_absent(self):
        for code, output, allowed in [(1, "HTTP/2.0 404 Not Found\n", True), (0, "HTTP/2.0 200 OK\n", False),
                                      (1, "HTTP/2.0 403 Forbidden\n", False), (1, "HTTP/2.0 500 Error\n", False), (1, "", False)]:
            with self.subTest(output=output), patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess([], code, output, "")):
                if allowed: release.absent("repos/ikzerok/worldedit/test")
                else:
                    with self.assertRaises(RuntimeError): release.absent("repos/ikzerok/worldedit/test")


class PaginationTests(unittest.TestCase):
    def test_paginated_ci_checks_every_page(self):
        path = "repos/ikzerok/worldedit/actions/workflows/ci.yml/runs?per_page=100"
        first = {"id": 1, "head_sha": E, "head_branch": "main", "event": "push", "run_number": 1,
                 "head_repository": {"full_name": release.EDITOR}, "status": "completed", "conclusion": "success",
                 "html_url": "https://example.invalid/ci"}
        second = dict(first, id=2, run_number=2, conclusion="failure")
        response = [{"total_count": 2, "workflow_runs": [first]}, {"total_count": 2, "workflow_runs": [second]}]
        with patch.object(release, "gh", return_value=json.dumps(response)) as gh:
            self.assertEqual(len(release.pages(path, collection="workflow_runs")), 2)
            with self.assertRaises(RuntimeError): release.ci(release.EDITOR, E)
            self.assertIn("--paginate", gh.call_args[0])
            self.assertIn("--slurp", gh.call_args[0])

    def test_paginated_issue_and_release_lists(self):
        for path in ["repos/ikzerok/worldedit/releases?per_page=100", "repos/ikzerok/worldline/issues?per_page=100"]:
            with patch.object(release, "gh", return_value='[[{"id":1}],[{"id":2}]]'):
                self.assertEqual(release.pages(path), [{"id": 1}, {"id": 2}])

    def test_api_pagination_failure_never_accepts_partial_output(self):
        error = subprocess.CalledProcessError(1, ["gh"], output='[[{"id":1}]]')
        with patch.object(release, "gh", side_effect=error):
            with self.assertRaises(subprocess.CalledProcessError):
                release.pages("repos/ikzerok/worldedit/releases?per_page=100")

    def test_truncated_malformed_duplicate_ci_pages_fail(self):
        bad = [[], [{"total_count": 2, "workflow_runs": [{"id": 1}]}],
               [{"total_count": 2, "workflow_runs": [{"id": 1}, {"id": 1}]}],
               [{"total_count": 0, "workflow_runs": {}}], [[{"id": 1}]],
               [{"total_count": 1, "workflow_runs": [{"id": 1}]}, {"total_count": 2, "workflow_runs": [{"id": 2}]}]]
        for response in bad:
            with self.subTest(response=response), patch.object(release, "gh", return_value=json.dumps(response)):
                with self.assertRaises(RuntimeError):
                    release.pages("repos/ikzerok/worldedit/actions/runs", collection="workflow_runs")

    def test_malicious_returned_hosts_are_not_api_endpoints(self):
        for url in ["https://evil.invalid/upload", "https://api.github.com.evil.invalid/repos/ikzerok/worldedit",
                    "https://uploads.github.com/repos/ikzerok/worldedit", "repos/attacker/repo/releases"]:
            with self.subTest(url=url), patch.object(release, "gh") as gh:
                with self.assertRaises(RuntimeError): release.api(url)
                gh.assert_not_called()


class WorkflowTests(unittest.TestCase):
    def test_minimum_permissions_and_no_push_pr_trigger(self):
        text = Path(__file__).parents[1].joinpath(".github/workflows/release.yml").read_text()
        self.assertIn("on:\n  create:\n", text)
        self.assertEqual(text.count("contents: write"), 1)
        self.assertEqual(text.count("persist-credentials: false"), text.count("uses: actions/checkout@"))
        self.assertNotIn("id-token:", text)
        self.assertNotIn("packages:", text)
        self.assertNotIn("actions: write", text)
        self.assertNotIn("pull_request_target", text)
        ci = Path(__file__).parents[1].joinpath(".github/workflows/ci.yml").read_text()
        self.assertIn("on: [push, pull_request]", ci)
        self.assertNotIn("contents: write", ci)
        self.assertIn("python-version: '3.12'", ci)
        for command in ["python -m unittest discover -s scripts -p 'test_check_pair.py' -v", "python scripts/release-test.py -v"]:
            self.assertIn(command + "\n          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }", ci)
        self.assertNotIn("--clobber", Path(__file__).with_name("release.py").read_text())


if __name__ == "__main__":
    unittest.main()
