#!/usr/bin/env python3
"""Run the pinned headless pair checks on Linux, macOS, or Windows."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys
import tempfile


class CheckFailure(RuntimeError):
    pass


class CheckNotStarted(CheckFailure):
    pass


def git(root, *args):
    return subprocess.check_output(
        ["git", "-C", str(root), *args], text=True, encoding="utf-8"
    ).strip()


def validate_pair(pair, actual):
    expected = pair.get("worldline", {}) if isinstance(pair, dict) else {}
    if not isinstance(expected, dict):
        expected = {}
    sha = expected.get("sha")
    if (expected.get("repository") != "ikzerok/worldline"
            or not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha)):
        raise CheckFailure("兼容记录必须指定 worldline 仓库和完整提交 SHA")
    if actual != expected["sha"]:
        raise CheckFailure("worldline HEAD 与 compatibility.json 不匹配，请在独立目录检出固定版本")


def checks(editor, language):
    steps = [("rustc", ["rustc", "-Vv"]), ("cargo", ["cargo", "-V"])]
    for name, root in [("worldline", language), ("worldedit", editor)]:
        steps.append((name + "-lines", [sys.executable, str(root / "scripts/check-source-lines.py")]))
    for name, root, workspace in [
        ("worldline", language, ["--workspace"]), ("worldedit", editor, [])
    ]:
        manifest = ["--manifest-path", str(root / "Cargo.toml")]
        steps.extend([
            (name + "-fmt", ["cargo", "fmt", *manifest, "--all", "--", "--check"]),
            (name + "-test", ["cargo", "test", *manifest, *workspace, "--locked"]),
            (name + "-clippy", ["cargo", "clippy", *manifest, *workspace, "--all-targets", "--locked", "--", "-D", "warnings"]),
            (name + "-build", ["cargo", "build", *manifest, *workspace, "--locked"]),
        ])
    manifest = ["--manifest-path", str(editor / "Cargo.toml")]
    target = ["--target", "wasm32-unknown-unknown", "--locked"]
    steps.extend([
        ("worldedit-wasm-clippy", ["cargo", "clippy", *manifest, *target, "--", "-D", "warnings"]),
        ("worldedit-wasm-build", ["cargo", "build", *manifest, *target]),
    ])
    feature = ["--features", "eds11_prototype", "--locked"]
    feature_target = ["--target", "wasm32-unknown-unknown", *feature]
    steps.extend([
        ("worldedit-prototype-test", ["cargo", "test", *manifest, *feature]),
        ("worldedit-prototype-clippy", ["cargo", "clippy", *manifest, "--all-targets", *feature, "--", "-D", "warnings"]),
        ("worldedit-prototype-build", ["cargo", "build", *manifest, *feature]),
        ("worldedit-prototype-wasm-clippy", ["cargo", "clippy", *manifest, *feature_target, "--", "-D", "warnings"]),
        ("worldedit-prototype-wasm-build", ["cargo", "build", *manifest, *feature_target]),
    ])
    return steps


def isolated_environment():
    """Keep each run's fixtures separate, without removing earlier evidence."""
    root = Path(tempfile.mkdtemp(prefix="worldedit-pair-")).resolve()
    environment = os.environ.copy()
    environment.update({key: str(root) for key in ("TMPDIR", "TMP", "TEMP")})
    return root, environment


def recorded(name, command, editor, evidence, environment=None):
    with (evidence / (name + ".log")).open("w", encoding="utf-8") as log:
        log.write(shlex.join(command) + "\n")
        log.flush()
        try:
            process = subprocess.Popen(
                command, cwd=editor, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, encoding="utf-8", errors="replace", env=environment,
            )
        except OSError as error:
            log.write(f"not started: {error}\n")
            raise CheckNotStarted(f"{name} 未启动：{error}") from error
        with process:
            for line in process.stdout:
                print(line, end="", flush=True)
                log.write(line)
            code = process.wait()
        log.write(f"exit code: {code}\n")
    if code:
        raise CheckFailure(f"{name} 失败，退出码：{code}")


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(editor, evidence):
    language = editor.parent / "worldline"
    results = [{"name": name, "status": "not_run"} for name, _ in checks(editor, language)]
    summary = {"status": "failed", "checks": results}
    try:
        pair = json.loads((editor / "compatibility.json").read_text(encoding="utf-8"))
        actual = git(language, "rev-parse", "HEAD")
        validate_pair(pair, actual)
        write_json(evidence / "compatibility.json", pair)
        temporary_root, child_environment = isolated_environment()
        environment = {
            "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "os": platform.platform(), "machine": platform.machine(),
            "cpu": platform.processor() or "not measured", "cpu_count": os.cpu_count(),
            "memory_bytes": "not measured", "gpu": "not measured",
            "worldedit": git(editor, "rev-parse", "HEAD"), "worldline": actual,
            "worldedit_working_tree": git(editor, "status", "--porcelain"),
            "worldline_working_tree": git(language, "status", "--porcelain"),
            "profile": "dev/test (Cargo defaults)",
            "temporary_directory": str(temporary_root),
            "temporary_directory_policy": "unique per run; TMPDIR/TMP/TEMP; retained; path recorded in evidence",
            "gui_acceptance": "not performed; headless Cargo checks only",
            "browser_version_dpi_ime": "not measured",
        }
        write_json(evidence / "environment.json", environment)
        write_json(evidence / "locks.json", [
            {"path": str(root / "Cargo.lock"), "sha256": hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest()}
            for root in [editor, language]
        ])
        for result, (name, command) in zip(results, checks(editor, language)):
            try:
                recorded(name, command, editor, evidence, child_environment)
            except CheckNotStarted as error:
                result["reason"] = str(error)
                raise
            except CheckFailure:
                result["status"] = "failed"
                raise
            result["status"] = "passed"
        summary["status"] = "passed"
    except (CheckFailure, OSError, ValueError, subprocess.CalledProcessError) as error:
        summary["error"] = str(error)
        print(error, file=sys.stderr)
    finally:
        write_json(evidence / "summary.json", summary)
    return 0 if summary["status"] == "passed" else 1


def main():
    editor = Path(__file__).resolve().parent.parent
    run_id = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    evidence = editor / "target" / "paired-check" / run_id
    evidence.mkdir(parents=True)
    print(f"配对检查证据：{evidence}", flush=True)
    return run(editor, evidence)


if __name__ == "__main__":
    sys.exit(main())
