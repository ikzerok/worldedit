#!/usr/bin/env python3
"""只从门禁固定的公开 Git 提交打包；不把本地目录或截图混入发行。"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


def snapshot(repository, name, sha, destination):
    release.require(subprocess.check_output(["git", "-C", str(repository), "rev-parse", "HEAD"],
                                           text=True, encoding="utf-8").strip() == sha, "检出 SHA 不匹配")
    subprocess.run(["git", "-C", str(repository), "archive", "--format=zip", "--prefix=" + name + "/",
                    "--output=" + str(destination), sha], check=True)
    with zipfile.ZipFile(destination) as archive:
        release.safe_archive(archive)


def packaged_cli_smoke(archive_path):
    # 输入和解压目录都只在临时目录；烟测不依赖、也不生成随发行分发的样例。
    with tempfile.TemporaryDirectory(prefix="worldedit-cli-smoke-") as directory:
        root = Path(directory)
        with zipfile.ZipFile(archive_path) as archive:
            release.safe_archive(archive)
            archive.extractall(root)
        source = root / "smoke.wl"
        source.write_text("event start\n  -> END\n", encoding="utf-8")
        result = subprocess.run([str(root / "windows/wl.exe"), "check", str(source), "--json"],
                                cwd=root, check=True, capture_output=True, text=True,
                                encoding="utf-8", timeout=60)
        report = json.loads(result.stdout)
        release.require(isinstance(report, dict) and report.get("ok") is True
                        and report.get("read_only") is False
                        and report.get("diagnostics") == []
                        and report.get("workspace_diagnostics") == []
                        and isinstance(report.get("stats"), dict)
                        and type(report["stats"].get("events")) is int
                        and report["stats"]["events"] == 1 and not result.stderr,
                        "已打包 CLI 的临时最小输入烟测结果不符")
        return {"command": "packaged wl.exe check <temporary>/smoke.wl --json",
                "input": "temporary generated event start with -> END; not distributed",
                "exit_code": result.returncode, "ok": report["ok"], "events": report["stats"]["events"],
                "read_only": report["read_only"], "diagnostics": 0, "workspace_diagnostics": 0}


def build(editor, core, pair_file, output):
    pair = json.loads(pair_file.read_text(encoding="utf-8"))
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="worldedit-release-") as directory:
        stage = Path(directory)
        for name, repository in [("worldedit", editor), ("worldline", core)]:
            archive_path = output / (name + "-source.zip")
            snapshot(repository, name, pair[name]["sha"], archive_path)
            with zipfile.ZipFile(archive_path) as archive:
                archive.extractall(stage)
        packages = stage / "packages"
        subprocess.run(["pwsh", "-NoProfile", "-File", str(stage / "worldedit/scripts/package.ps1"),
                        "-OutputDirectory", str(packages)], cwd=stage / "worldedit", check=True)
        builds = list(packages.iterdir())
        release.require(len(builds) == 1, "打包目录不唯一")
        packaged = builds[0]
        for name in ["worldedit-windows-x64.zip", "worldedit-web.zip"]:
            shutil.copyfile(packaged / name, output / name)
        # Exercise the executable read back from the final Windows ZIP.
        cli_smoke = packaged_cli_smoke(output / "worldedit-windows-x64.zip")
    pair["build"] = {"platform": "windows-x64", "web": "trunk release --locked",
                     "cli_smoke": cli_smoke,
                     "ui_scope": "本次 CI 打包验证资产完整性及已打包 CLI 烟测；原生交互验收另见独立验收报告，CI 构建不代表 Windows/macOS 原生交互已验收"}
    (output / "release-pair.json").write_text(json.dumps(pair, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (output / "SHA256SUMS.txt").write_text("".join(
        f"{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n"
        for name in sorted(release.ASSETS - {"SHA256SUMS.txt"})), encoding="utf-8")
    release.verify_assets(output, pair)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor", type=Path, required=True)
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--pair", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build(args.editor.resolve(), args.core.resolve(), args.pair.resolve(), args.output.resolve())
