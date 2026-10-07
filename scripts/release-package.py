#!/usr/bin/env python3
"""Windows 打包辅助：共用发行审计规则，精确排除作品样例而保留可构建源码。"""
import argparse
import importlib.util
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import zipfile

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

LOCAL_NAMES = release.PRIVATE_DIRECTORIES | {".idea", ".vscode", ".zcode",
                                             ".ds_store", "thumbs.db"}
ROOT_LOCAL_NAMES = {"dist", "releases"}
LOCAL_SUFFIXES = {".log", ".tmp", ".bak", ".swp", ".pyc", ".pem", ".key", ".pfx", ".p12", ".keystore"}


def excluded(parts):
    lower = [part.casefold() for part in parts]
    relative = release.package_relative_parts(lower)
    return (release.distributed_sample(lower)
            or bool(relative) and relative[0] in ROOT_LOCAL_NAMES
            or any(part in LOCAL_NAMES or part.startswith(".env")
                   or release.PRIVATE_FILES.fullmatch(part) for part in lower)
            or lower[-1].endswith(".save.json") or Path(lower[-1]).suffix in LOCAL_SUFFIXES)


def require_regular_path(path):
    attributes = getattr(path.lstat(), "st_file_attributes", 0)
    release.require(not path.is_symlink() and not attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT,
                    f"源码目录不能包含链接：{path}")


def copy_public(source, destination, prefix):
    """prefix 为归档内完整相对路径，不按任意 examples/fixtures 名称误删源码。"""
    require_regular_path(source)
    destination.mkdir(parents=True, exist_ok=True)
    for path in sorted(source.iterdir()):
        relative = prefix + "/" + path.name
        if excluded(relative.split("/")):
            continue
        require_regular_path(path)
        release.safe_entry(zipfile.ZipInfo(relative + ("/" if path.is_dir() else "")))
        target = destination / path.name
        if path.is_dir():
            copy_public(path, target, relative)
        else:
            release.require(path.is_file(), f"源码含特殊文件：{path}")
            shutil.copy2(path, target)


def audit(path):
    with zipfile.ZipFile(path) as archive:
        release.safe_archive(archive)
        release.require(archive.testzip() is None, f"ZIP 损坏：{path.name}")


def check_source(source):
    require_regular_path(source)
    if (source / ".git").exists():
        # 本地打包先检查，再在归档前复查；避免工作树二进制配上旧 HEAD 源码。
        status = subprocess.check_output(["git", "-C", str(source), "status", "--porcelain=v1",
                                          "--untracked-files=all", "--ignore-submodules=none"])
        release.require(not status, f"源码工作树有未提交或未跟踪更改，不能与 HEAD 归档配对：{source}")


def source_archive(source, destination, name):
    release.require(name in {"worldedit", "worldline"}, "源码归档根目录不符")
    require_regular_path(source)
    if (source / ".git").exists():
        # 有 Git 时严格使用提交快照和其 export-ignore，绝不重写 git archive 结果。
        check_source(source)
        subprocess.run(["git", "-C", str(source), "archive", "--format=zip", "--prefix=" + name + "/",
                        "--output=" + str(destination), "HEAD"], check=True)
    else:
        # 从已导出的源码构建时没有 .git；仍保留 worldline/worldedit 同级根目录。
        with tempfile.TemporaryDirectory(prefix="worldedit-public-source-") as directory:
            root = Path(directory) / name
            copy_public(source, root, name)
            with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(root.rglob("*")):
                    if path.is_file():
                        archive.write(path, path.relative_to(root.parent).as_posix())
    audit(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    copy = subparsers.add_parser("copy")
    source = subparsers.add_parser("source")
    for command in [copy, source]:
        command.add_argument("--source", type=Path, required=True)
        command.add_argument("--destination", type=Path, required=True)
    copy.add_argument("--prefix", required=True)
    source.add_argument("--name", required=True)
    check = subparsers.add_parser("audit")
    check.add_argument("archives", nargs="+", type=Path)
    preflight = subparsers.add_parser("check-source")
    preflight.add_argument("sources", nargs="+", type=Path)
    args = parser.parse_args()
    if args.command == "copy":
        copy_public(args.source.absolute(), args.destination.resolve(), args.prefix)
    elif args.command == "source":
        source_archive(args.source.absolute(), args.destination.resolve(), args.name)
    elif args.command == "check-source":
        for path in args.sources:
            check_source(path.absolute())
    else:
        for path in args.archives:
            audit(path)


if __name__ == "__main__":
    main()
