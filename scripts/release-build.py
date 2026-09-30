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
        # Exercise the packaged CLI, not the checkout/debug executable.
        subprocess.run([str(packaged / "windows/wl.exe"), "check",
                        str(packaged / "windows/worldline/examples/harbor-world"), "--json"], check=True)
    pair["build"] = {"platform": "windows-x64", "web": "trunk release --locked",
                     "cli_smoke": "packaged wl check harbor-world --json exited 0",
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
