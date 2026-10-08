"""发行协议离线测试：不读凭据、不联网、不创建 tag/release。"""
import base64
import copy
import hashlib
import importlib.util
import io
import json
import os
import re
import shutil
import stat
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from urllib.parse import urlsplit
from unittest.mock import patch
import warnings
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
        self.assertIn("ci 字段", result["acceptance"])
        self.assertIn("独立版本更新报告", result["acceptance"])
        self.assertNotIn("仓库文本记录", result["acceptance"])
        self.assertEqual(result["ci"][release.EDITOR]["url"], "https://github.com/example/actions/runs/99")

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
        file.write_text(file.read_text(encoding="utf-8") * 2, encoding="utf-8")
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


    def test_distributed_samples_rejected_in_every_final_zip(self):
        roots = {"worldedit-source.zip": "worldedit", "worldline-source.zip": "worldline",
                 "worldedit-windows-x64.zip": "windows", "worldedit-web.zip": "web"}
        samples = ["examples/harbor-world/world.wl", "spec/examples/project.json", "samples/demo.wl",
                   "docs/design/editor-system/20260926/eds10-visual-sample.html"]
        for name, root in roots.items():
            for sample in samples:
                path = root + "/" + sample
                with self.subTest(archive=name, path=path):
                    self.files[name][path] = b"sample"
                    self.write()
                    with self.assertRaisesRegex(RuntimeError, "发行禁止的样例"):
                        release.verify_assets(self.root, self.pair)
                    del self.files[name][path]
        for path in ["windows/worldline/examples/minimal.wl", "windows/worldline/spec/examples/project.json",
                     "windows/worldline/EXAMPLES/", "web/worldline/spec/Examples/"]:
            name = "worldedit-web.zip" if path.startswith("web/") else "worldedit-windows-x64.zip"
            with self.subTest(path=path):
                self.files[name][path] = b""
                self.write()
                with self.assertRaisesRegex(RuntimeError, "发行禁止的样例"):
                    release.verify_assets(self.root, self.pair)
                del self.files[name][path]

    def test_source_tools_fixtures_templates_and_required_assets_remain_allowed(self):
        for path in ["core/examples/relations_profile.rs", "core/tests/fixtures/examples/world.wl",
                     "core/tests/catalog.rs", "cli/tests/cli.rs", "runtime/src/lib.rs",
                     "spec/templates.catalog.json", "spec/schema/worldline.schema.json", "Cargo.lock"]:
            self.files["worldline-source.zip"]["worldline/" + path] = b"public source"
        for path in ["src/app/tests.rs", "tests/fixtures/samples/world.wl", "assets/fonts/OFL.txt",
                     "assets/licenses/self-cell-APACHE.txt", ".agent/skills/worldedit-authoring/SKILL.md",
                     "docs/examples.md", "index.html", "build.rs", "Cargo.lock"]:
            self.files["worldedit-source.zip"]["worldedit/" + path] = b"public source"
        self.write()
        release.verify_assets(self.root, self.pair)


class ReleaseVisibilityTests(unittest.TestCase):
    def setUp(self):
        self.draft = {"id": 77, "tag_name": "v0.4.0", "draft": True, "target_commitish": E}

    def test_new_draft_waits_one_two_four_seconds_with_fixed_id_checks(self):
        with patch.object(release, "api", return_value=self.draft) as api, \
             patch.object(release, "pages", side_effect=[[], [], [], [self.draft]]), \
             patch.object(release.time, "sleep") as sleep:
            release.release_set("v0.4.0", 77, E)
            self.assertEqual([c.args[0] for c in sleep.call_args_list], [1, 2, 4])
            self.assertEqual(api.call_count, 4)
            self.assertTrue(all(c.args == (f"repos/{release.EDITOR}/releases/77",) for c in api.call_args_list))

    def test_missing_draft_stops_after_bounded_wait(self):
        with patch.object(release, "api", return_value=self.draft), \
             patch.object(release, "pages", return_value=[]), patch.object(release.time, "sleep") as sleep:
            with self.assertRaises(RuntimeError): release.release_set("v0.4.0", 77, E)
            self.assertEqual(sleep.call_count, 3)

    def test_duplicate_or_changed_identity_never_retries(self):
        for listing, fixed in [([self.draft, self.draft], self.draft),
                               ([dict(self.draft, id=78)], self.draft),
                               ([], dict(self.draft, id=78)),
                               ([], dict(self.draft, draft=False)),
                               ([], dict(self.draft, target_commitish=C)),
                               ([], dict(self.draft, tag_name="v9.9.9"))]:
            with self.subTest(listing=listing, fixed=fixed), \
                 patch.object(release, "api", return_value=fixed), \
                 patch.object(release, "pages", return_value=listing), patch.object(release.time, "sleep") as sleep:
                with self.assertRaises(RuntimeError): release.release_set("v0.4.0", 77, E)
                sleep.assert_not_called()


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
        (self.repo / ".agent/README.md").write_text("public text", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        self.sha = self.git("rev-parse", "HEAD").strip()
        spec = importlib.util.spec_from_file_location("release_build", Path(__file__).with_name("release-build.py"))
        self.builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.builder)

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], text=True, encoding="utf-8")

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


    def test_export_ignore_keeps_exact_git_archive_and_buildable_source_paths(self):
        fixtures = {"examples/harbor-world/world.wl": "sample", "spec/examples/project.json": "{}",
                    "docs/design/editor-system/20260926/eds10-visual-sample.html": "sample",
                    "core/examples/relations_profile.rs": "fn main() {}",
                    "core/tests/fixtures/examples/world.wl": "event start\n  -> END\n",
                    "spec/templates.catalog.json": "{}", "Cargo.toml": "[workspace]\n",
                    ".gitattributes": "/examples export-ignore\n/spec/examples export-ignore\n"
                                      "/docs/design/editor-system/20260926/eds10-visual-sample.html export-ignore\n"}
        for name, text in fixtures.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "sample-free fixture")
        sha = self.git("rev-parse", "HEAD").strip()
        output = self.root / "source.zip"
        self.builder.snapshot(self.repo, "worldline", sha, output)
        raw_archive = subprocess.check_output(["git", "-C", str(self.repo), "archive", "--format=zip",
                                               "--prefix=worldline/", sha])
        self.assertEqual(output.read_bytes(), raw_archive)
        with zipfile.ZipFile(output) as archive:
            names = set(archive.namelist())
            self.assertEqual(archive.comment.decode("ascii"), sha)
            for name in fixtures:
                if name.startswith(("examples/", "spec/examples/")) or name.endswith("eds10-visual-sample.html"):
                    self.assertNotIn("worldline/" + name, names)
                else:
                    self.assertIn("worldline/" + name, names)

    def test_snapshot_without_sample_export_ignore_fails_closed(self):
        (self.repo / "examples").mkdir()
        (self.repo / "examples/world.wl").write_text("event start\n  -> END\n", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "unexcluded sample fixture")
        sha = self.git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(RuntimeError, "发行禁止的样例"):
            self.builder.snapshot(self.repo, "worldline", sha, self.root / "source.zip")


class PackageSourceTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("release_package", Path(__file__).with_name("release-package.py"))
        self.packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.packager)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def write_files(self, root, files):
        for name in files:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("public fixture\n", encoding="utf-8")

    def test_exported_source_fallback_preserves_sibling_roots_and_test_tools(self):
        kept = ["Cargo.toml", "Cargo.lock", "core/src/lib.rs", "core/examples/relations_profile.rs",
                "core/tests/fixtures/examples/world.wl", "spec/templates.catalog.json",
                "spec/schema/worldline.schema.json", ".agent/skills/authoring/SKILL.md", "index.html",
                "docs/qa/fixtures/replay/world.wl", "docs/releases/v0.28.0.md",
                "core/tests/fixtures/dist/input.json"]
        omitted = ["examples/world.wl", "spec/examples/project.json",
                   "docs/design/editor-system/20260926/eds10-visual-sample.html",
                   "target/cache", "node_modules/cache", ".env", "__pycache__/cache.pyc",
                   "releases/local.zip", "dist/index.html"]
        for name in ["worldline", "worldedit"]:
            with self.subTest(repository=name):
                source = self.root / name
                self.write_files(source, kept + omitted)
                output = self.root / (name + "-source.zip")
                self.packager.source_archive(source, output, name)
                with zipfile.ZipFile(output) as archive:
                    self.assertEqual(set(archive.namelist()), {name + "/" + path for path in kept})
                    self.assertTrue(all(path.startswith(name + "/") for path in archive.namelist()))
                    archive.extractall(self.root / "restored")
        self.assertTrue((self.root / "restored/worldline/Cargo.toml").is_file())
        self.assertTrue((self.root / "restored/worldedit/Cargo.toml").is_file())

    def test_desktop_docs_copy_excludes_spec_examples_and_standalone_visual_sample(self):
        source = self.root / "source"
        self.write_files(source, ["examples/project.json", "templates.catalog.json", "language.md"])
        destination = self.root / "windows/worldline/spec"
        self.packager.copy_public(source, destination, "windows/worldline/spec")
        self.assertFalse((destination / "examples").exists())
        self.assertTrue((destination / "templates.catalog.json").is_file())
        self.assertTrue((destination / "language.md").is_file())
        self.write_files(source, ["design/editor-system/20260926/eds10-visual-sample.html", "handbook.md",
                                  "releases/v0.28.0.md"])
        docs = self.root / "windows/docs"
        self.packager.copy_public(source, docs, "windows/docs")
        self.assertFalse((docs / "design/editor-system/20260926/eds10-visual-sample.html").exists())
        self.assertTrue((docs / "handbook.md").is_file())
        self.assertTrue((docs / "releases/v0.28.0.md").is_file())

    def test_windows_junctions_fail_before_source_or_child_directory_traversal(self):
        original_lstat, original_iterdir = Path.lstat, Path.iterdir
        for location in ["root", "child"]:
            with self.subTest(location=location):
                source = self.root / location
                junction = source if location == "root" else source / "junction"
                junction.mkdir(parents=True)
                (junction / "outside.txt").write_text("must not read", encoding="utf-8")
                destination = self.root / (location + "-copy")
                def lstat(path, *args, **kwargs):
                    if path == junction:
                        return SimpleNamespace(st_mode=stat.S_IFDIR | 0o755,
                                               st_file_attributes=stat.FILE_ATTRIBUTE_DIRECTORY
                                               | stat.FILE_ATTRIBUTE_REPARSE_POINT)
                    return original_lstat(path, *args, **kwargs)
                def iterdir(path):
                    self.assertNotEqual(path, junction, "不得遍历 junction 指向的目录")
                    return original_iterdir(path)
                with patch.object(Path, "lstat", new=lstat), patch.object(Path, "iterdir", new=iterdir), \
                     patch.object(self.packager.shutil, "copy2") as copy_file:
                    self.assertFalse(junction.is_symlink(), "夹具必须模拟不是普通符号链接的 Windows junction")
                    with self.assertRaisesRegex(RuntimeError, "不能包含链接"):
                        self.packager.copy_public(source, destination, "worldline")
                    copy_file.assert_not_called()
                self.assertFalse((destination / "outside.txt").exists())
                self.assertFalse((destination / "junction").exists())

    def test_source_root_symlink_rejected_without_cli_resolution_bypass(self):
        source = self.root / "outside"
        self.write_files(source, ["private.txt"])
        link = self.root / "linked-source"
        try:
            link.symlink_to(source, target_is_directory=True)
        except (NotImplementedError, OSError) as error:
            self.skipTest(f"当前测试账户不能创建符号链接：{error}")
        destination = self.root / "copied"
        commands = [
            ["copy", "--source", str(link), "--destination", str(destination), "--prefix", "worldline"],
            ["source", "--source", str(link), "--destination", str(self.root / "source.zip"), "--name", "worldline"],
            ["check-source", str(link)],
        ]
        for command in commands:
            with self.subTest(command=command[0]), patch("sys.argv", ["release-package.py", *command]):
                with self.assertRaisesRegex(RuntimeError, "不能包含链接"):
                    self.packager.main()
        self.assertFalse(destination.exists())
        self.assertFalse((self.root / "source.zip").exists())

    def create_checkout(self):
        source = self.root / "worldline"
        self.write_files(source, ["Cargo.toml", "core/examples/relations_profile.rs", "examples/world.wl"])
        (source / ".gitattributes").write_text("/examples export-ignore\n", encoding="utf-8")
        (source / ".gitignore").write_text("/target/\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q", str(source)], check=True)
        for arguments in [["config", "user.name", "Offline test"], ["config", "user.email", "test@example.invalid"],
                          ["add", "."], ["commit", "-qm", "fixture"]]:
            subprocess.run(["git", "-C", str(source), *arguments], check=True)
        return source

    def test_clean_checkout_source_uses_exact_git_archive_and_ignores_local_build_cache(self):
        source = self.create_checkout()
        self.write_files(source, ["target/local-build-output"])
        output = self.root / "worldline-source.zip"
        self.packager.source_archive(source, output, "worldline")
        expected = subprocess.check_output(["git", "-C", str(source), "archive", "--format=zip",
                                            "--prefix=worldline/", "HEAD"])
        self.assertEqual(output.read_bytes(), expected)

    def test_dirty_checkout_source_fails_before_archive_creation(self):
        source = self.create_checkout()
        output = self.root / "worldline-source.zip"
        for name in ["Cargo.toml", "untracked.txt"]:
            with self.subTest(change=name):
                (source / name).write_text("uncommitted", encoding="utf-8")
                with self.assertRaisesRegex(RuntimeError, "未提交或未跟踪"):
                    self.packager.check_source(source)
                with self.assertRaisesRegex(RuntimeError, "未提交或未跟踪"):
                    self.packager.source_archive(source, output, "worldline")
                self.assertFalse(output.exists())
                if name == "Cargo.toml":
                    subprocess.run(["git", "-C", str(source), "checkout", "--", name], check=True)
                else:
                    (source / name).unlink()
        (source / "Cargo.toml").write_text("staged change", encoding="utf-8")
        subprocess.run(["git", "-C", str(source), "add", "Cargo.toml"], check=True)
        with self.assertRaisesRegex(RuntimeError, "未提交或未跟踪"):
            self.packager.source_archive(source, output, "worldline")
        self.assertFalse(output.exists())

    def test_package_script_uses_shared_filter_and_audits_every_zip_before_hashing(self):
        text = Path(__file__).with_name("package.ps1").read_text(encoding="utf-8-sig")
        self.assertIn("foreach ($directory in @('spec', 'docs'))", text)
        self.assertNotIn("@('spec', 'docs', 'examples')", text)
        self.assertIn("Run-PackageHelper @('source'", text)
        self.assertLess(text.index("Run-PackageHelper @('check-source'"), text.index("Run-Cargo @('build'"))
        self.assertIn("Run-PackageHelper (@('audit') + $archives)", text)
        self.assertLess(text.index("Run-PackageHelper (@('audit')"), text.index("Get-FileHash"))


class PackagedSmokeTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("release_build", Path(__file__).with_name("release-build.py"))
        self.builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.builder)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        self.archive = self.output / "worldedit-windows-x64.zip"
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr("windows/wl.exe", b"packaged executable fixture")
        self.report = {"ok": True, "read_only": False, "diagnostics": [], "workspace_diagnostics": [],
                       "stats": {"events": 1}}

    def test_reads_packaged_binary_and_generates_only_temporary_minimal_input(self):
        inputs = []
        def run(command, **kwargs):
            executable, verb, source, option = command
            self.assertEqual((verb, option), ("check", "--json"))
            self.assertEqual(Path(executable).read_bytes(), b"packaged executable fixture")
            source = Path(source)
            inputs.append(source)
            self.assertFalse(source.is_relative_to(self.output))
            self.assertEqual(source.read_text(encoding="utf-8"), "event start\n  -> END\n")
            self.assertEqual(kwargs["cwd"], source.parent)
            self.assertTrue(kwargs["check"])
            self.assertTrue(kwargs["capture_output"])
            self.assertEqual(kwargs["encoding"], "utf-8")
            self.assertEqual(kwargs["timeout"], 60)
            return subprocess.CompletedProcess(command, 0, json.dumps(self.report), "")
        with patch.object(self.builder.subprocess, "run", side_effect=run):
            result = self.builder.packaged_cli_smoke(self.archive)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["events"], 1)
        self.assertTrue(result["ok"])
        self.assertIn("not distributed", result["input"])
        self.assertTrue(all(not path.exists() for path in inputs))
        self.assertEqual(list(self.output.iterdir()), [self.archive])

    def test_bad_json_or_semantically_wrong_success_is_rejected(self):
        bad = [[], {}, dict(self.report, ok=False), dict(self.report, ok=1), dict(self.report, read_only=True),
               dict(self.report, diagnostics=[{"severity": "warning"}]),
               dict(self.report, workspace_diagnostics=[{}]), dict(self.report, stats={"events": 0}),
               dict(self.report, stats={"events": True})]
        for report in bad:
            with self.subTest(report=report), patch.object(self.builder.subprocess, "run",
                    return_value=subprocess.CompletedProcess([], 0, json.dumps(report), "")):
                with self.assertRaises(RuntimeError): self.builder.packaged_cli_smoke(self.archive)
        with patch.object(self.builder.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "not json", "")):
            with self.assertRaises(json.JSONDecodeError): self.builder.packaged_cli_smoke(self.archive)

    def test_nonzero_exit_timeout_and_stderr_fail_without_publishing_input(self):
        for error in [subprocess.CalledProcessError(1, ["wl.exe"]), subprocess.TimeoutExpired(["wl.exe"], 60)]:
            with self.subTest(error=error), patch.object(self.builder.subprocess, "run", side_effect=error):
                with self.assertRaises(type(error)): self.builder.packaged_cli_smoke(self.archive)
        with patch.object(self.builder.subprocess, "run",
                          return_value=subprocess.CompletedProcess([], 0, json.dumps(self.report), "unexpected warning")):
            with self.assertRaises(RuntimeError): self.builder.packaged_cli_smoke(self.archive)
        self.assertEqual(list(self.output.iterdir()), [self.archive])

    def test_sample_contamination_is_rejected_before_extraction_or_execution(self):
        with zipfile.ZipFile(self.archive, "a") as archive:
            archive.writestr("windows/worldline/examples/world.wl", "event start\n  -> END\n")
        with patch.object(self.builder.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "发行禁止的样例"):
                self.builder.packaged_cli_smoke(self.archive)
            run.assert_not_called()


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
        text = Path(__file__).parents[1].joinpath(".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("on:\n  create:\n", text)
        self.assertEqual(text.count("contents: write"), 1)
        self.assertEqual(text.count("persist-credentials: false"), text.count("uses: actions/checkout@"))
        self.assertNotIn("id-token:", text)
        self.assertNotIn("packages:", text)
        self.assertNotIn("actions: write", text)
        self.assertNotIn("pull_request_target", text)
        ci = Path(__file__).parents[1].joinpath(".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn("on: [push, pull_request]", ci)
        self.assertNotIn("contents: write", ci)
        self.assertNotIn("actions/upload-artifact", ci)
        self.assertNotIn("worldedit/target/paired-check/", ci)
        self.assertIn("python-version: '3.12'", ci)
        for command in ["python -X warn_default_encoding -W error::EncodingWarning -m unittest discover -s scripts -p 'test_check_pair.py' -v", "python -X warn_default_encoding -W error::EncodingWarning scripts/release-test.py -v"]:
            self.assertIn(command + "\n          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }", ci)
        self.assertNotIn("--clobber", Path(__file__).with_name("release.py").read_text(encoding="utf-8"))

    def test_ci_and_release_share_pinned_trunk_installer(self):
        root = Path(__file__).parents[1]
        for workflow in ["ci.yml", "release.yml"]:
            text = (root / ".github/workflows" / workflow).read_text(encoding="utf-8")
            with self.subTest(workflow=workflow):
                self.assertEqual(text.count("scripts/install-trunk.py\n"), 1)
                self.assertEqual(text.count("scripts/install-trunk.py --verify-path\n"), 1)
                self.assertNotIn("cargo install trunk", text)
                self.assertLess(text.index("scripts/install-trunk.py\n"),
                                text.index("scripts/install-trunk.py --verify-path\n"))


class TrunkInstallTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("install_trunk", Path(__file__).with_name("install-trunk.py"))
        self.installer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.installer)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.path_file = self.root / "github-path"
        self.path_file.write_text("", encoding="utf-8")
        self.binary = b"offline fixture, never execute"
        for mocked in [patch.dict(os.environ, {"RUNNER_TEMP": str(self.root), "GITHUB_PATH": str(self.path_file)}),
                       patch.object(self.installer.platform, "system", return_value="Windows"),
                       patch.object(self.installer.platform, "machine", return_value="AMD64"),
                       patch.object(self.installer, "EXE_SHA256", hashlib.sha256(self.binary).hexdigest())]:
            mocked.start()
            self.addCleanup(mocked.stop)
        self.run = patch.object(self.installer.subprocess, "run", return_value=subprocess.CompletedProcess(
            [], 0, "trunk 0.21.14\n", "")).start()
        self.addCleanup(patch.stopall)

    def archive(self, names=("trunk.exe",), attributes=0):
        buffer = io.BytesIO()
        with warnings.catch_warnings(), zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
            warnings.simplefilter("ignore", UserWarning)  # duplicate-name fixture
            for name in names:
                entry = zipfile.ZipInfo(name)
                entry.external_attr = attributes
                archive.writestr(entry, self.binary)
        return buffer.getvalue()

    def unpack(self, data):
        with patch.object(self.installer, "ZIP_SHA256", hashlib.sha256(data).hexdigest()):
            return self.installer.unpack_verified(data)

    def install(self, data=None):
        if data is None:
            data = self.archive()
        with patch.object(self.installer, "download", return_value=data), \
             patch.object(self.installer, "ZIP_SHA256", hashlib.sha256(data).hexdigest()):
            self.installer.install()

    def test_official_pins(self):
        self.assertEqual(self.installer.VERSION, "0.21.14")
        self.assertEqual(self.installer.URL, "https://github.com/trunk-rs/trunk/releases/download/v0.21.14/"
                         "trunk-x86_64-pc-windows-msvc.zip")
        self.assertEqual(self.installer.ZIP_SHA256,
                         "cd6ac15b9daff0365e5695036791ef2ce3c63f61c014f5a8c532363266e4569c")
        source = Path(__file__).with_name("install-trunk.py").read_text(encoding="utf-8")
        self.assertIn("209f218e2c01516ef4e59b97ab73b879ff2180c73f4187fa152b15532e8eecae", source)

    def test_only_verified_binary_is_written_and_executed_before_path(self):
        def version(command, **kwargs):
            self.assertEqual(self.path_file.read_text(encoding="utf-8"), "")
            self.assertEqual(command, [str(self.installer.installed_path()), "--version"])
            self.assertTrue(kwargs["check"])
            self.assertEqual(kwargs["timeout"], 30)
            return subprocess.CompletedProcess(command, 0, "trunk 0.21.14\n", "")
        self.run.side_effect = version
        self.install()
        executable = self.installer.installed_path()
        self.assertEqual(executable.read_bytes(), self.binary)
        self.assertEqual(list(executable.parent.iterdir()), [executable])
        self.assertEqual(self.path_file.read_text(encoding="utf-8"), str(executable.parent) + "\n")

    def test_wrong_zip_hash_stops_before_parse_write_or_execute(self):
        with patch.object(self.installer, "download", return_value=self.archive()), \
             patch.object(self.installer.zipfile, "ZipFile") as archive:
            with self.assertRaisesRegex(RuntimeError, "ZIP SHA256"):
                self.installer.install()
            archive.assert_not_called()
        self.run.assert_not_called()
        self.assertFalse(self.installer.installed_path().parent.exists())
        self.assertEqual(self.path_file.read_text(encoding="utf-8"), "")

    def test_wrong_executable_hash_stops_before_write_or_execute(self):
        with patch.object(self.installer, "EXE_SHA256", "0" * 64):
            with self.assertRaisesRegex(RuntimeError, "EXE SHA256"):
                self.install()
        self.run.assert_not_called()
        self.assertFalse(self.installer.installed_path().parent.exists())

    def test_root_single_entry_only(self):
        for names in [("../trunk.exe",), ("sub/trunk.exe",), ("C:/trunk.exe",), ("\\trunk.exe",),
                      ("other.exe",), ("trunk.exe/",), (), ("trunk.exe", "extra.txt"),
                      ("trunk.exe", "trunk.exe")]:
            with self.subTest(names=names), self.assertRaises(RuntimeError):
                self.unpack(self.archive(names))

    def test_links_directories_and_special_files_rejected(self):
        for attributes in [stat.S_IFLNK << 16, stat.S_IFIFO << 16, stat.S_IFDIR << 16, 0x10, 0x400]:
            with self.subTest(attributes=attributes), self.assertRaises(RuntimeError):
                self.unpack(self.archive(attributes=attributes))

    def test_archive_and_decompressed_size_bounds(self):
        data = self.archive()
        for name, limit in [("MAX_ZIP_BYTES", len(data) - 1), ("MAX_EXE_BYTES", len(self.binary) - 1)]:
            with self.subTest(name=name), patch.object(self.installer, name, limit), self.assertRaises(RuntimeError):
                self.unpack(data)
        with self.assertRaises(RuntimeError):
            self.installer.read_bounded(io.BytesIO(b"12345"), 4)
        with self.assertRaises(RuntimeError):
            self.installer.read_bounded(io.BytesIO(b""), 4)

    def test_bad_version_or_exit_never_updates_path(self):
        for stdout, stderr in [("trunk 0.21.13\n", ""), ("trunk 0.21.14 extra\n", ""),
                               (" trunk 0.21.14\n", ""), ("trunk 0.21.14\n", "warning")]:
            self.run.return_value = subprocess.CompletedProcess([], 0, stdout, stderr)
            with self.subTest(stdout=stdout, stderr=stderr), self.assertRaises(RuntimeError):
                self.install()
            self.assertEqual(self.path_file.read_text(encoding="utf-8"), "")
            self.assertFalse(self.installer.installed_path().parent.exists())
        self.run.side_effect = subprocess.CalledProcessError(1, ["fixture"])
        with self.assertRaises(subprocess.CalledProcessError):
            self.install()
        self.assertEqual(self.path_file.read_text(encoding="utf-8"), "")

    def test_following_step_verifies_path_hash_and_version(self):
        self.install()
        executable = self.installer.installed_path()
        with patch.object(self.installer.shutil, "which", return_value=str(executable)):
            self.installer.verify_path()
            self.assertEqual(self.run.call_count, 2)
            executable.write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "SHA256"):
                self.installer.verify_path()
            self.assertEqual(self.run.call_count, 2)
        for actual in [None, str(self.path_file)]:
            with self.subTest(actual=actual), patch.object(self.installer.shutil, "which", return_value=actual):
                with self.assertRaisesRegex(RuntimeError, "PATH"):
                    self.installer.verify_path()

    def test_success_logs_support_strict_cp1252_with_unicode_paths(self):
        runner_temp = self.root / "CI-中文路径"
        runner_temp.mkdir()
        buffer = io.BytesIO()
        with io.TextIOWrapper(buffer, encoding="cp1252", errors="strict") as stdout, \
             patch("sys.stdout", stdout), patch.dict(os.environ, {"RUNNER_TEMP": str(runner_temp)}):
            self.install()
            executable = self.installer.installed_path()
            with patch.object(self.installer.shutil, "which", return_value=str(executable)):
                self.installer.verify_path()
            stdout.flush()
            log = buffer.getvalue().decode("ascii")
        self.assertIn("Verified official Trunk 0.21.14", log)
        self.assertIn(ascii(str(executable)), log)
        self.assertEqual(self.run.call_count, 2)
        self.assertEqual(executable.read_bytes(), self.binary)
        self.assertEqual(self.path_file.read_text(encoding="utf-8"), str(executable.parent) + "\n")

    def test_download_rejects_untrusted_redirect_and_http(self):
        for url in ["http://github.com/file", "https://evil.invalid/file", "https://github.com.evil.invalid/file",
                    "https://user:secret@github.com/file", "https://github.com:444/file"]:
            with self.subTest(url=url), self.assertRaises(RuntimeError):
                self.installer.OfficialRedirect().redirect_request(None, None, 302, "Found", {}, url)
        for url in [self.installer.URL, "https://release-assets.githubusercontent.com/file?signature=fixture"]:
            self.installer.check_url(url)

    def test_download_has_no_credentials_and_checks_response_size(self):
        for length, body, allowed in [("3", b"zip", True), (str(self.installer.MAX_ZIP_BYTES + 1), b"x", False),
                                      (None, b"x" * 5, False)]:
            response = io.BytesIO(body)
            response.status, response.headers = 200, {} if length is None else {"Content-Length": length}
            response.geturl = lambda: self.installer.URL
            with self.subTest(length=length), patch.object(self.installer.urllib.request, "build_opener") as build, \
                 patch.dict(os.environ, {"GH_TOKEN": "fixture-secret", "GITHUB_TOKEN": "fixture-secret"}), \
                 patch.object(self.installer, "MAX_ZIP_BYTES", 4):
                build.return_value.open.return_value = response
                if allowed:
                    self.assertEqual(self.installer.download(), body)
                else:
                    with self.assertRaises(RuntimeError):
                        self.installer.download()
                request = build.return_value.open.call_args.args[0]
                self.assertEqual(request.get_method(), "GET")
                self.assertEqual(request.header_items(), [("User-agent", "worldedit-trunk-bootstrap")])
                self.assertEqual(build.call_args.args[0].proxies, {})

    def test_unsupported_platform_stops_before_download(self):
        with patch.object(self.installer.platform, "system", return_value="Linux"), \
             patch.object(self.installer, "download") as download:
            with self.assertRaises(RuntimeError):
                self.installer.install()
            download.assert_not_called()

    def test_existing_install_not_reused(self):
        self.install()
        previous = self.path_file.read_text(encoding="utf-8")
        with self.assertRaises(FileExistsError):
            self.install()
        self.assertEqual(self.run.call_count, 1)
        self.assertEqual(self.path_file.read_text(encoding="utf-8"), previous)


class RuntimeDependencyNoticeTests(unittest.TestCase):
    def test_self_cell_apache_notice_is_fixed_and_copied_to_both_packages(self):
        root = Path(__file__).resolve().parent.parent
        notice = root / "assets/licenses/self-cell-APACHE.txt"
        self.assertEqual(hashlib.sha256(notice.read_bytes()).hexdigest(),
                         "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4")
        package = (root / "scripts/package.ps1").read_text(encoding="utf-8-sig")
        for destination in ["$desktopRoot", "$webRoot"]:
            self.assertIn('Copy-Item -LiteralPath "$editorRoot/assets/licenses/self-cell-APACHE.txt" '
                          f"-Destination (Join-Path {destination} 'SELF-CELL-LICENSE.txt')", package)


class ReleaseNotesTests(unittest.TestCase):
    def test_current_release_notes_have_only_absolute_document_links(self):
        root = Path(__file__).resolve().parent.parent
        manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
        version = re.search(r'^version\s*=\s*"([^"]+)"', manifest, re.MULTILINE).group(1)
        notes = root / "docs" / "releases" / ("v" + version + ".md")
        self.assertTrue(notes.is_file(), "当前版本必须提供实际发行说明")
        body = release.release_body("v" + version)
        links = re.findall(r'\[[^\]]+\]\(([^\s)]+)\)', body)
        self.assertTrue(links, "发行说明应提供完整产品用法入口")
        for link in links:
            parsed = urlsplit(link)
            self.assertEqual(parsed.scheme, "https", f"Release页面不能使用相对文档链接：{link}")
            self.assertTrue(parsed.netloc, f"文档链接必须含实际站点：{link}")

    def test_version_notes_disclose_compatibility_and_do_not_read_arbitrary_paths(self):
        body = release.release_body("v0.8.0")
        self.assertIn("负数rnd旧错误行为例外", body)
        self.assertIn("有效非负固定seed", body)
        self.assertIn("Windows/macOS原生交互", body)
        self.assertIn("SHA256SUMS.txt", release.release_body("v99.0.0"))
        with self.assertRaises(RuntimeError):
            release.release_body("../../private")


if __name__ == "__main__":
    unittest.main()
