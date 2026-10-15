"""发行Markdown只改副本，跨仓链接离线可用，源码引用固定提交。"""
import ast
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile


SPEC = importlib.util.spec_from_file_location("package_doc_links", Path(__file__).with_name("package-doc-links.py"))
helper = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = helper
SPEC.loader.exec_module(helper)
EDITOR_SHA = "a" * 40
CORE_SHA = "b" * 40


class PackageDocLinksTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.editor = self.root / "sources/worldedit"
        self.core = self.root / "sources/worldline"
        self.desktop = self.root / "release/windows"
        for path in (self.editor, self.core, self.desktop):
            path.mkdir(parents=True)

    def source(self, repo, path, body):
        root = self.editor if repo == "worldedit" else self.core
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(body.encode("utf-8") if isinstance(body, str) else body)
        return target

    def staged(self, repo, path, body):
        source = self.source(repo, path, body)
        target = self.desktop / ("worldline" if repo == "worldline" else "") / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        return target

    def run_map(self, check=False, **kwargs):
        return helper.remap(self.desktop, self.editor, self.core,
                            kwargs.get("editor_sha", EDITOR_SHA), CORE_SHA, check)

    def run_script_cli(self, script, encoding, *arguments):
        environment = os.environ.copy()
        environment.update(PYTHONIOENCODING=f"{encoding}:strict", PYTHONUTF8="0")
        return subprocess.run(
            [sys.executable, str(Path(helper.__file__).with_name(script)), *arguments],
            capture_output=True, env=environment, check=False, timeout=30,
        )

    def run_cli(self, encoding, *arguments):
        return self.run_script_cli(
            "package-doc-links.py", encoding, "--desktop-root", str(self.desktop),
            "--editor-source", str(self.editor), "--worldline-source", str(self.core),
            "--editor-sha", EDITOR_SHA, "--worldline-sha", CORE_SHA, *arguments,
        )

    def test_cross_repository_links_follow_nested_desktop_layout(self):
        editor = self.staged("worldedit", "docs/start.md", "[契约](../../worldline/spec/workspace.md#边界)\n")
        core = self.staged("worldline", "spec/workspace.md", "# 边界\n[启动](../../worldedit/docs/start.md)\n")
        result = self.run_map()
        self.assertEqual(result["local_rewrites"], 2)
        self.assertIn("[契约](../worldline/spec/workspace.md#边界)", editor.read_text(encoding="utf-8"))
        self.assertIn("[启动](../../docs/start.md)", core.read_text(encoding="utf-8"))

    def test_missing_application_content_becomes_fixed_online_source(self):
        page = self.staged("worldedit", "docs/guide.md", "[源码](../src/main.rs)\n[样例](../../worldline/examples/story/world.wl)\n")
        self.source("worldedit", "src/main.rs", "fn main() {}")
        self.source("worldline", "examples/story/world.wl", "event start\n  -> END\n")
        result = self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertEqual(result["online_rewrites"], 2)
        self.assertIn(f"/worldedit/blob/{EDITOR_SHA}/src/main.rs", body)
        self.assertIn(f"/worldline/blob/{CORE_SHA}/examples/story/world.wl", body)
        self.assertIn("需要联网", body)
        self.assertFalse((self.desktop / "src").exists())
        self.assertFalse((self.desktop / "worldline/examples").exists())

    def test_packaged_license_and_icon_aliases(self):
        body = "[字体](assets/fonts/OFL.txt) [渲染](assets/licenses/resvg-MIT.txt) [图标](assets/worldedit.ico)"
        page = self.staged("worldedit", "README.md", body)
        for original, packed in (("assets/fonts/OFL.txt", "FONT-LICENSE.txt"),
                                 ("assets/licenses/resvg-MIT.txt", "RESVG-LICENSE.txt"),
                                 ("assets/worldedit.ico", "worldedit.ico")):
            self.source("worldedit", original, "original bytes")
            (self.desktop / packed).write_bytes(b"original bytes")
        self.run_map()
        actual = page.read_text(encoding="utf-8")
        self.assertIn("[字体](FONT-LICENSE.txt)", actual)
        self.assertIn("[渲染](RESVG-LICENSE.txt)", actual)
        self.assertIn("[图标](worldedit.ico)", actual)

    def test_main_urls_use_packaged_target_or_fixed_commit(self):
        page = self.staged("worldedit", "README.md",
                           "[指南](https://github.com/ikzerok/worldedit/blob/main/docs/guide.md#范围)\n"
                           "[脚本](https://github.com/ikzerok/worldedit/blob/main/scripts/tool.py)\n")
        self.staged("worldedit", "docs/guide.md", "# 范围\n")
        self.source("worldedit", "scripts/tool.py", "pass\n")
        self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertIn("[指南](docs/guide.md#范围)", body)
        self.assertIn(f"/blob/{EDITOR_SHA}/scripts/tool.py", body)
        self.assertNotIn("/blob/main/", body)

    def test_historical_commit_and_other_external_urls_are_preserved(self):
        historical = "https://github.com/ikzerok/worldline/blob/" + "c" * 40 + "/spec/old.md"
        tagged = "https://github.com/ikzerok/worldedit/blob/v0.29.0/docs/maps.md"
        short = "https://github.com/ikzerok/worldedit/blob/5e44564/docs/old.html"
        page = self.staged("worldedit", "README.md",
                           f"[历史]({historical}) [旧版]({tagged}) [旧提交]({short}) [外部](https://example.com/docs?q=1#x)")
        self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertIn(historical, body)
        self.assertIn(tagged, body)
        self.assertIn(short, body)
        self.assertIn("需要联网", body)
        self.assertIn("https://example.com/docs?q=1#x", body)

    def test_balanced_parentheses_titles_unicode_and_reference_definitions(self):
        page = self.staged("worldedit", "README.md",
                           '[一](docs/a(b).md "标题")\n[二](<docs/中文 空格.md>)\n[三][ref]\n[ref]: docs/a(b).md "保持标题"\n')
        self.staged("worldedit", "docs/a(b).md", "# 内容\n")
        self.staged("worldedit", "docs/中文 空格.md", "# 内容\n")
        self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertIn('[一](docs/a%28b%29.md "标题")', body)
        self.assertIn("[二](<docs/%E4%B8%AD%E6%96%87%20%E7%A9%BA%E6%A0%BC.md>)", body)
        self.assertIn('[ref]: docs/a%28b%29.md "保持标题"', body)

    def test_code_examples_and_same_document_anchors_are_not_rewritten(self):
        body = "[本节](#边界)\n`[不是链接](missing.md)`\n```md\n[也不是](private.md)\n```\n    [缩进代码](hidden.md)\n"
        page = self.staged("worldedit", "README.md", body)
        self.run_map()
        self.assertIn(body, page.read_text(encoding="utf-8"))

    def test_undistributed_image_is_a_source_link_not_a_broken_embedding(self):
        page = self.staged("worldedit", "README.md", "![历史图](assets/old.png)\n")
        self.source("worldedit", "assets/old.png", b"image")
        self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertIn(f"[历史图](https://github.com/ikzerok/worldedit/blob/{EDITOR_SHA}/assets/old.png)", body)
        self.assertNotIn("![历史图]", body)

    def test_directory_source_links_use_tree_url(self):
        page = self.staged("worldedit", "README.md", "[目录](src/)\n")
        self.source("worldedit", "src/main.rs", "fn main() {}")
        self.run_map()
        self.assertIn(f"/tree/{EDITOR_SHA}/src)", page.read_text(encoding="utf-8"))

    def test_export_ignored_targets_work_without_git_or_sample_files(self):
        page = self.staged("worldedit", "README.md",
                           "[历史页](docs/design/editor-system/20260926/eds10-visual-sample.html)\n"
                           "[旧例](../worldline/examples/catalog-import/README.md)\n"
                           "[语法例](../worldline/spec/examples/)\n"
                           "[目录](https://github.com/ikzerok/worldline/tree/main/examples/blue-hour-reservoir)\n")
        result = self.run_map()
        body = page.read_text(encoding="utf-8")
        self.assertEqual(result["online_rewrites"], 4)
        self.assertIn(f"/blob/{EDITOR_SHA}/docs/design/editor-system/20260926/eds10-visual-sample.html", body)
        self.assertIn(f"/blob/{CORE_SHA}/examples/catalog-import/README.md", body)
        self.assertIn(f"/tree/{CORE_SHA}/spec/examples", body)
        self.assertIn(f"/tree/{CORE_SHA}/examples/blue-hour-reservoir", body)
        self.assertFalse((self.core / "examples").exists())
        self.assertFalse((self.editor / ".git").exists())

    def test_dry_run_and_apply_preserve_source_and_non_markdown_bytes(self):
        page = self.staged("worldedit", "README.md", "[源码](src/main.rs)")
        self.source("worldedit", "src/main.rs", "fn main() {}")
        binary = self.desktop / "worldedit.exe"
        binary.write_bytes(b"PE unchanged")
        before = {p: p.read_bytes() for p in self.editor.rglob("*") if p.is_file()}
        self.assertEqual(self.run_map(check=True)["mode"], "check")
        self.assertEqual(page.read_bytes(), before[self.editor / "README.md"])
        self.run_map()
        self.assertEqual({p: p.read_bytes() for p in before}, before)
        self.assertEqual(binary.read_bytes(), b"PE unchanged")

    def test_filtered_source_zips_can_rebuild_application_documentation(self):
        spec = importlib.util.spec_from_file_location("release_package_docs_test",
                                                    Path(__file__).with_name("release-package.py"))
        package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(package)
        page = self.staged("worldedit", "README.md",
                           "[契约](../worldline/spec/README.md)\n"
                           "[历史例](../worldline/examples/story/world.wl)\n"
                           "[规范例](../worldline/spec/examples/README.md)\n"
                           "[视觉页](docs/design/editor-system/20260926/eds10-visual-sample.html)\n")
        self.staged("worldline", "spec/README.md", "# 契约\n")
        self.source("worldline", "examples/story/world.wl", "event start\n  -> END\n")
        self.source("worldline", "spec/examples/README.md", "# 样例\n")
        self.source("worldedit", "docs/design/editor-system/20260926/eds10-visual-sample.html", "<html></html>")
        exported = self.root / "unzipped"
        archives = {}
        for source in (self.editor, self.core):
            path = self.root / (source.name + ".zip")
            package.source_archive(source, path, source.name)
            archives[path] = path.read_bytes()
            with zipfile.ZipFile(path) as archive:
                archive.extractall(exported)
        self.editor, self.core = exported / "worldedit", exported / "worldline"
        source_bytes = {p: p.read_bytes() for p in exported.rglob("*") if p.is_file()}
        self.assertFalse((self.core / "examples").exists())
        self.assertFalse((self.core / "spec/examples").exists())
        self.assertFalse((self.editor / "docs/design/editor-system/20260926/eds10-visual-sample.html").exists())
        result = self.run_map()
        self.assertEqual(result["online_rewrites"], 3)
        self.assertIn("[契约](worldline/spec/README.md)", page.read_text(encoding="utf-8"))
        self.assertIn("需要联网", page.read_text(encoding="utf-8"))
        self.assertEqual({p: p.read_bytes() for p in source_bytes}, source_bytes)
        self.assertEqual({p: p.read_bytes() for p in archives}, archives)
        self.assertFalse(any(p.suffix == ".wl" for p in self.desktop.rglob("*")))

    def test_missing_targets_outside_exact_export_ignored_scopes_fail(self):
        page = self.staged("worldedit", "README.md", "")
        for target in ("docs/missing.md", "examples/story/world.wl",
                       "../worldline/docs/examples/missing.md", "../worldline/examples-extra/missing.md",
                       "docs/design/editor-system/20260926/other-visual-sample.html",
                       "https://github.com/ikzerok/worldline/blob/main/spec/missing.md",
                       f"https://github.com/ikzerok/worldedit/blob/{EDITOR_SHA}/src/missing.rs"):
            with self.subTest(target=target):
                page.write_text(f"[不存在]({target})\n", encoding="utf-8")
                before = page.read_bytes()
                with self.assertRaisesRegex(ValueError, "目标缺失"):
                    self.run_map()
                self.assertEqual(page.read_bytes(), before)

    def test_idempotence_and_different_commit_rejection(self):
        page = self.staged("worldedit", "README.md", "[指南](docs/guide.md)\n")
        self.staged("worldedit", "docs/guide.md", "text")
        self.run_map()
        body = page.read_bytes()
        second = self.run_map()
        self.assertEqual(second["rewritten_files"], 0)
        self.assertEqual(second["already_mapped"], 2)
        self.assertEqual(page.read_bytes(), body)
        with self.assertRaisesRegex(ValueError, "不同提交"):
            self.run_map(editor_sha="d" * 40)

    def test_mapped_pages_recheck_missing_local_targets(self):
        page = self.staged("worldedit", "README.md", "[指南](docs/guide.md)\n")
        guide = self.staged("worldedit", "docs/guide.md", "text")
        self.run_map()
        before = page.read_bytes()
        guide.unlink()
        with self.assertRaisesRegex(ValueError, "发行文档链接目标缺失"):
            self.run_map()
        self.assertEqual(page.read_bytes(), before)

    def test_yaml_frontmatter_bom_and_crlf_are_preserved(self):
        original = '\ufeff---\r\nname: authoring\r\n---\r\n# 技能\r\n[源码](tool.rs)\r\n'
        page = self.staged("worldedit", ".agent/SKILL.md", original)
        self.source("worldedit", ".agent/tool.rs", "// tool")
        self.run_map()
        body = page.read_bytes().decode("utf-8")
        self.assertTrue(body.startswith('\ufeff---\r\nname: authoring\r\n---\r\n<!--'))
        self.assertNotIn("\n", body.replace("\r\n", ""))

    def test_all_links_validate_before_any_file_changes(self):
        first = self.staged("worldedit", "A.md", "valid document\n")
        self.staged("worldedit", "Z.md", "[坏目标](missing.md)\n")
        with self.assertRaisesRegex(ValueError, "目标缺失"):
            self.run_map()
        self.assertEqual(first.read_text(encoding="utf-8"), "valid document\n")

    def test_requires_full_shas_and_separate_output_root(self):
        self.staged("worldedit", "README.md", "text")
        with self.assertRaisesRegex(ValueError, "40位"):
            self.run_map(editor_sha="main")
        with self.assertRaisesRegex(ValueError, "不得互相包含"):
            helper.remap(self.editor, self.editor, self.core, EDITOR_SHA, CORE_SHA)

    def test_rejects_escape_and_file_urls(self):
        page = self.staged("worldedit", "README.md", "[越界](../../outside.md)")
        with self.assertRaisesRegex(ValueError, "边界"):
            self.run_map()
        page.write_text("[本机](file:///private/data)", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "file链接"):
            self.run_map()

    def test_rejects_staged_symlink_and_hardlink_to_source(self):
        source = self.source("worldedit", "README.md", "text")
        staged = self.desktop / "README.md"
        try:
            staged.symlink_to(source)
        except OSError:
            self.skipTest("当前平台没有创建符号链接权限")
        with self.assertRaisesRegex(ValueError, "链接"):
            self.run_map()
        staged.unlink()
        os.link(source, staged)
        with self.assertRaisesRegex(ValueError, "硬链接"):
            self.run_map()
        self.assertEqual(source.read_bytes(), b"text")

    def test_rejects_source_target_symlink(self):
        self.staged("worldedit", "README.md", "[链接](private.md)")
        outside = self.root / "outside.md"
        outside.write_bytes(b"private")
        try:
            (self.editor / "private.md").symlink_to(outside)
        except OSError:
            self.skipTest("当前平台没有创建符号链接权限")
        with self.assertRaisesRegex(ValueError, "链接"):
            self.run_map()

    def test_rejects_unreferenced_staged_directory_link(self):
        self.staged("worldedit", "README.md", "text")
        try:
            (self.desktop / "outside").symlink_to(self.core, target_is_directory=True)
        except OSError:
            self.skipTest("当前平台没有创建符号链接权限")
        with self.assertRaisesRegex(ValueError, "链接"):
            self.run_map()

    def test_cli_check_outputs_structured_plan(self):
        page = self.staged("worldedit", "README.md", "text\n")
        result = subprocess.run([sys.executable, str(Path(helper.__file__)), "--desktop-root", str(self.desktop),
                                 "--editor-source", str(self.editor), "--worldline-source", str(self.core),
                                 "--editor-sha", EDITOR_SHA, "--worldline-sha", CORE_SHA, "--check"],
                                capture_output=True, text=True, encoding="utf-8", check=True)
        self.assertEqual(json.loads(result.stdout)["markdown_files"], 1)
        self.assertEqual(page.read_bytes(), b"text\n")

    def test_cli_unicode_reports_round_trip_with_legacy_stdout(self):
        page = self.staged(
            "worldedit", "docs/中文指南.md",
            '\ufeff---\r\n标题: 中文指南\r\n---\r\n# 中文指南\r\n'
            '[本地](本地目标.md#中文锚点🌟)\r\n'
            '[源码](../src/实现.rs#源码锚点)\r\n'
            '[契约](../../worldline/spec/契约.md#边界)\r\n'
            '[本节](#中文锚点)\r\n',
        )
        self.staged("worldedit", "docs/本地目标.md", "# 中文锚点\n[返回](中文指南.md#中文指南)\n")
        self.staged("worldline", "spec/契约.md", "# 边界\n")
        self.source("worldedit", "src/实现.rs", "// 源码锚点\n")
        (self.desktop / "非文档.bin").write_bytes(b"\x00\xff unchanged")
        source_bytes = {p: p.read_bytes() for root in (self.editor, self.core)
                        for p in root.rglob("*") if p.is_file()}
        staged_bytes = {p: p.read_bytes() for p in self.desktop.rglob("*") if p.is_file()}
        mapper = helper.Remapper(self.desktop, self.editor, self.core, EDITOR_SHA, CORE_SHA)
        pending, report = mapper.plan()
        expected_bytes = {**staged_bytes, **dict(pending)}
        self.assertEqual(report["local_rewrites"], 3)
        self.assertEqual(report["online_rewrites"], 1)
        self.assertIn({"file": "docs/中文指南.md", "line": 5,
                       "before": "本地目标.md#中文锚点🌟",
                       "after": "%E6%9C%AC%E5%9C%B0%E7%9B%AE%E6%A0%87.md#中文锚点🌟",
                       "kind": "local"}, report["changes"])
        for encoding in ("cp1252", "ascii", "utf-8"):
            with self.subTest(encoding=encoding):
                for path, data in staged_bytes.items():
                    path.write_bytes(data)
                for mode, arguments in (("check", ("--check",)), ("apply", ())):
                    with self.subTest(mode=mode):
                        result = self.run_cli(encoding, *arguments)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertEqual(result.stderr, b"")
                        # ASCII JSON escapes are lossless, independent of the pipe's encoding.
                        output = result.stdout.decode("ascii")
                        self.assertIn("\\u4e2d", output)
                        self.assertIn("\\ud83c\\udf1f", output)
                        self.assertEqual(json.loads(output), {"ok": True, "mode": mode, **report})
                        self.assertEqual({p: p.read_bytes() for p in staged_bytes},
                                         staged_bytes if mode == "check" else expected_bytes)
                        self.assertEqual({p: p.read_bytes() for p in source_bytes}, source_bytes)
                body = page.read_bytes().decode("utf-8")
                self.assertTrue(body.startswith('\ufeff---\r\n标题: 中文指南\r\n---\r\n<!--'))
                self.assertIn("#中文锚点)", body)
                self.assertIn("需要联网", body)
                self.assertNotIn("\n", body.replace("\r\n", ""))
                for arguments in (("--check",), ()):
                    result = self.run_cli(encoding, *arguments)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stderr, b"")
                    checked = json.loads(result.stdout.decode("ascii"))
                    self.assertEqual(checked["already_mapped"], 3)
                    self.assertEqual(checked["rewritten_files"], 0)
                    self.assertEqual(checked["changes"], [])
                    self.assertEqual({p: p.read_bytes() for p in staged_bytes}, expected_bytes)

    def test_cli_unicode_errors_keep_original_stderr_diagnostics(self):
        self.staged("worldedit", "A.md", "先验证全部文件，再修改发行副本\n")
        self.staged("worldedit", "docs/错误路径.md", "[不存在](缺失目标.md#中文锚点)\n")
        before = {p: p.read_bytes() for p in self.root.rglob("*") if p.is_file()}
        diagnostic = f"ValueError: 源码文档链接目标缺失：worldedit/{Path('docs') / '缺失目标.md'}"
        for encoding in ("cp1252", "ascii", "utf-8"):
            for arguments in (("--check",), ()):
                with self.subTest(encoding=encoding, arguments=arguments):
                    result = self.run_cli(encoding, *arguments)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, b"")
                    # Python's stderr keeps backslashreplace even with PYTHONIOENCODING=:strict.
                    error = result.stderr.decode(encoding)
                    self.assertIn(diagnostic.encode(encoding, "backslashreplace").decode(encoding), error)
                    self.assertIn("Traceback (most recent call last):", error)
                    self.assertNotIn("UnicodeEncodeError", error)
                    self.assertNotIn("lost sys.stderr", error)
                    self.assertEqual({p: p.read_bytes() for p in before}, before)

    def test_cli_unicode_argument_errors_remain_visible(self):
        for encoding in ("cp1252", "ascii", "utf-8"):
            with self.subTest(encoding=encoding):
                result = self.run_cli(encoding, "--未知选项")
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                error = result.stderr.decode(encoding)
                self.assertIn("--未知选项".encode(encoding, "backslashreplace").decode(encoding), error)
                self.assertNotIn("UnicodeEncodeError", error)

    def test_cli_help_supports_legacy_stdout(self):
        for encoding in ("cp1252", "ascii", "utf-8"):
            with self.subTest(encoding=encoding):
                result = self.run_cli(encoding, "--help")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stderr, b"")
                output = result.stdout.decode(encoding)
                self.assertIn("usage:", output)
                self.assertIn(helper.__doc__.encode(encoding, "backslashreplace").decode(encoding), output)
                self.assertIn("--check", output)

    def test_cli_invalid_utf8_document_is_not_swallowed(self):
        page = self.staged("worldedit", "错误字节.md", b"\xff invalid UTF-8")
        for encoding in ("cp1252", "ascii", "utf-8"):
            for arguments in (("--check",), ()):
                with self.subTest(encoding=encoding, arguments=arguments):
                    result = self.run_cli(encoding, *arguments)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, b"")
                    error = result.stderr.decode(encoding)
                    self.assertIn("UnicodeDecodeError", error)
                    self.assertNotIn("UnicodeEncodeError", error)
                    self.assertEqual(page.read_bytes(), b"\xff invalid UTF-8")

    def test_distribution_cli_help_supports_legacy_stdout(self):
        for script in ("release.py", "release-build.py", "release-package.py"):
            source = Path(helper.__file__).with_name(script).read_text(encoding="utf-8")
            description = ast.get_docstring(ast.parse(source))
            for encoding in ("cp1252", "ascii", "utf-8"):
                with self.subTest(script=script, encoding=encoding):
                    result = self.run_script_cli(script, encoding, "--help")
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stderr, b"")
                    output = result.stdout.decode(encoding)
                    self.assertIn("usage:", output)
                    self.assertIn(description.encode(encoding, "backslashreplace").decode(encoding), output)

    def test_distribution_cli_unicode_file_errors_remain_visible(self):
        missing_pair = self.root / "缺失配对.json"
        missing_zip = self.root / "缺失发行包.zip"
        output = self.root / "不可创建的发行目录"
        cases = (
            ("release.py", missing_pair, ["verify", "--pair", str(missing_pair)]),
            ("release-build.py", missing_pair,
             ["--editor", str(self.editor), "--core", str(self.core),
              "--pair", str(missing_pair), "--output", str(output)]),
            ("release-package.py", missing_zip, ["audit", str(missing_zip)]),
        )
        for script, missing, arguments in cases:
            for encoding in ("cp1252", "ascii", "utf-8"):
                with self.subTest(script=script, encoding=encoding):
                    # These paths fail on local reads, before build/gate/publish actions.
                    result = self.run_script_cli(script, encoding, *arguments)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, b"")
                    error = result.stderr.decode(encoding)
                    diagnostic = repr(str(missing)).encode(encoding, "backslashreplace").decode(encoding)
                    self.assertIn(diagnostic, error)
                    self.assertIn("FileNotFoundError", error)
                    self.assertNotIn("UnicodeEncodeError", error)
                    self.assertNotIn("lost sys.stderr", error)
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
