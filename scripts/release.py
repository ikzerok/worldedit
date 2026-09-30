#!/usr/bin/env python3
"""受限发行门禁与发布。只在 release.yml 的 publish job 使用写权限。"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import unicodedata
import subprocess
import tempfile
import time
import tomllib
import zipfile

EDITOR = "ikzerok/worldedit"
CORE = "ikzerok/worldline"
ZIP_NAMES = {"worldedit-windows-x64.zip", "worldedit-web.zip",
             "worldedit-source.zip", "worldline-source.zip"}
ASSETS = ZIP_NAMES | {"release-pair.json", "SHA256SUMS.txt"}
SHA = re.compile(r"[0-9a-f]{40}")
BRANCH = re.compile(r"release/(v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*))")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def gh(*args, payload=None):
    command = ["gh", *args]
    if payload is not None:
        command += ["--input", "-"]
    return subprocess.check_output(command, input=json.dumps(payload) if payload is not None else None,
                                   text=True, encoding="utf-8")


def endpoint(path):
    require(isinstance(path, str) and path.startswith((f"repos/{EDITOR}/", f"repos/{CORE}/"))
            and "://" not in path and "\\" not in path and ".." not in path.split("/"),
            "只允许固定 GitHub 仓库相对 API 路径，不接受返回的外部 URL")
    return path


def api(path, method="GET", payload=None):
    endpoint(path)
    return json.loads(gh("api", "--method", method, path, payload=payload))


def pages(path, collection=None):
    endpoint(path)
    result = json.loads(gh("api", "--paginate", "--slurp", path))
    require(isinstance(result, list) and result, "分页响应无效")
    output = []
    totals = []
    for page in result:
        if collection is not None:
            require(isinstance(page, dict) and isinstance(page.get("total_count"), int), "CI 分页响应无效")
            totals.append(page["total_count"])
            page = page.get(collection)
        require(isinstance(page, list) and all(isinstance(item, dict) for item in page), "分页条目无效")
        output.extend(page)
    if collection is not None:
        require(len(set(totals)) == 1 and totals[0] == len(output), "CI 分页不完整或查询期间发生变化")
        require(len({item["id"] for item in output}) == len(output), "CI 分页包含重复运行")
    return output


def release_set(tag, expected_id=None, target_sha=None):
    # 新建 draft 的列表可见性允许有限延迟；固定 ID 身份每轮仍须核验。
    for attempt in range(4 if target_sha is not None else 1):
        if target_sha is not None:
            fixed = api(f"repos/{EDITOR}/releases/{expected_id}")
            require(fixed.get("id") == expected_id and fixed.get("tag_name") == tag
                    and fixed.get("target_commitish") == target_sha and fixed.get("draft") is True,
                    "新建 draft 的身份、来源或状态已改变，停止")
        releases = pages(f"repos/{EDITOR}/releases?per_page=100")
        require(all(isinstance(item.get("id"), int) and isinstance(item.get("tag_name"), str)
                    and isinstance(item.get("draft"), bool) for item in releases), "Release 分页条目缺少必要字段")
        matches = [item for item in releases if item["tag_name"] == tag]
        if expected_id is None:
            require(not matches, "已有同名 draft 或公开 Release，停止且不覆盖")
            return
        if matches:
            require(len(matches) == 1 and matches[0].get("id") == expected_id,
                    "同名 Release 不唯一或 ID 改变，停止")
            return
        require(target_sha is not None and attempt < 3, "新建 draft 未在有界等待内可见，停止")
        time.sleep(2 ** attempt)


def absent(path):
    endpoint(path)
    # Only an explicit HTTP 404 means absent. Auth/network/5xx errors fail closed.
    result = subprocess.run(["gh", "api", "--include", path], capture_output=True,
                            text=True, encoding="utf-8")
    require(result.returncode != 0 and re.search(r"^HTTP/\S+ 404\b", result.stdout, re.M),
            f"发行目标已存在，或无法确认不存在；人工审查后处理：{path}")


def content(repo, sha, path):
    item = api(f"repos/{repo}/contents/{path}?ref={sha}")
    require(item.get("encoding") == "base64", f"无法读取 {path}")
    return base64.b64decode(item["content"]).decode("utf-8-sig")


def main_sha(repo):
    sha = api(f"repos/{repo}/git/ref/heads/main")["object"]["sha"]
    require(SHA.fullmatch(sha), "main SHA 格式错误")
    return sha


def ci(repo, sha):
    runs = pages(f"repos/{repo}/actions/workflows/ci.yml/runs?branch=main&event=push&head_sha={sha}&per_page=100", collection="workflow_runs")
    exact = [r for r in runs if r.get("head_sha") == sha and r.get("head_branch") == "main"
             and r.get("event") == "push" and r.get("head_repository", {}).get("full_name") == repo]
    require(exact, f"{repo}@{sha} 尚无 main push CI")
    latest = max(exact, key=lambda r: (r["run_number"], r.get("run_attempt", 1)))
    require(latest["status"] == "completed" and latest["conclusion"] == "success",
            f"{repo} 最新精确 main CI 尚未成功")
    return {"id": latest["id"], "url": latest["html_url"], "sha": sha}


def context():
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    require(os.environ.get("GITHUB_EVENT_NAME") == "create", "仅接受 create 事件")
    require(os.environ.get("GITHUB_REPOSITORY") == EDITOR
            and event.get("repository", {}).get("full_name") == EDITOR, "仅接受正式仓库")
    require(event.get("ref_type") == "branch", "仅接受创建发行分支")
    branch = event.get("ref", "")
    match = BRANCH.fullmatch(branch)
    require(match is not None, "分支必须为 release/vX.Y.Z，无前导零或预发行后缀")
    sha = os.environ.get("GITHUB_SHA", "")
    require(SHA.fullmatch(sha), "事件 SHA 格式错误")
    require(os.environ.get("GITHUB_REF") == "refs/heads/" + branch, "事件 ref 不一致")
    return branch, match[1], sha


def gate(existing=False):
    branch, tag, editor_sha = context()
    require(main_sha(EDITOR) == editor_sha, "发行 SHA 不再是 editor main")
    require(api(f"repos/{EDITOR}/git/ref/heads/{branch}")["object"]["sha"] == editor_sha,
            "发行分支已移动")
    editor_version = tomllib.loads(content(EDITOR, editor_sha, "Cargo.toml"))["package"]["version"]
    require(tag == "v" + editor_version, "发行版本与 Cargo.toml 不符")
    pair = json.loads(content(EDITOR, editor_sha, "compatibility.json"))["worldline"]
    require(pair.get("repository") == CORE and SHA.fullmatch(pair.get("sha", "")), "无效配对")
    core_sha = pair["sha"]
    require(main_sha(CORE) == core_sha, "固定 worldline SHA 不再是其 main")
    core_version = tomllib.loads(content(CORE, core_sha, "Cargo.toml"))["workspace"]["package"]["version"]
    evidence = {}
    for repo, sha in [(EDITOR, editor_sha), (CORE, core_sha)]:
        issues = pages(f"repos/{repo}/issues?state=open&per_page=100")
        require(not [i for i in issues if "pull_request" not in i], f"{repo} 仍有未关闭 issue")
        evidence[repo] = ci(repo, sha)
    if not existing:
        absent(f"repos/{EDITOR}/git/ref/tags/{tag}")
        absent(f"repos/{EDITOR}/releases/tags/{tag}")
    else:
        ref = api(f"repos/{EDITOR}/git/ref/tags/{tag}")["object"]
        require(ref.get("type") == "commit" and ref.get("sha") == editor_sha, "新标签配对不符")
    # Catch main changes during the API inspection too; the final PATCH is not atomic with refs.
    require(main_sha(EDITOR) == editor_sha and main_sha(CORE) == core_sha, "门禁查询期间 main 已变动")
    return {"schema_version": 1, "tag": tag, "worldedit": {"repository": EDITOR, "sha": editor_sha, "version": editor_version},
            "worldline": {"repository": CORE, "sha": core_sha, "version": core_version}, "ci": evidence,
            "acceptance": "两仓 open issues 为零；自动校验见本记录 ci 字段中的精确 main push CI 链接。人工与 AI 验收结论通过独立版本更新报告提供，原始验收记录不随源码发布。"}


QA_TEXT_EXTENSIONS = {".md", ".txt", ".json", ".jsonl", ".log", ".csv", ".tsv", ".yaml", ".yml",
                      ".toml", ".html", ".css", ".js", ".cjs", ".mjs", ".py", ".sh", ".ps1", ".rs", ".wl"}
PRIVATE_DIRECTORIES = {".git", ".tooling", ".aws", ".ssh", ".azure", ".gnupg", "node_modules", "target", "__pycache__"}
PRIVATE_FILES = re.compile(r"^(?:credentials?|secrets?|tokens?)(?:[._-].*)?$", re.I)
ENCODED_MEDIA = re.compile(rb"data:(?:image|video|audio)/|iVBORw0KGgo|/9j/4[A-Za-z0-9+/]|R0lGOD[dl]h|UklGR[A-Za-z0-9+/]{2,12}V0VCUA", re.I)


def safe_entry(item):
    name = item.filename
    require(name and name == item.orig_filename and name == unicodedata.normalize("NFC", name) and "\\" not in name
            and not name.startswith("/") and ":" not in name and "%" not in name
            and all(ord(c) >= 32 and ord(c) != 127 for c in name), "ZIP 条目不是规范安全路径")
    parts = name.rstrip("/").split("/")
    require(all(p and p not in {".", ".."} and p == p.rstrip(" .") for p in parts), "ZIP 路径别名或越界")
    lower = [p.casefold() for p in parts]
    require(not any(re.fullmatch(r"(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?", p) for p in lower), "ZIP 含 Windows 保留设备名")
    require(not any(p in PRIVATE_DIRECTORIES for p in lower), "ZIP 含私有/缓存目录")
    require(not any(p.startswith(".env") or PRIVATE_FILES.fullmatch(p) for p in lower), "ZIP 含凭据或私有配置路径")
    require(Path(lower[-1]).suffix not in {".pem", ".key", ".pfx", ".p12", ".keystore"}, "ZIP 含密钥文件")
    mode = item.external_attr >> 16
    require(not stat.S_ISLNK(mode) and stat.S_IFMT(mode) in {0, stat.S_IFREG, stat.S_IFDIR}, "ZIP 含链接或特殊文件")
    qa = any(lower[i:i + 2] == ["docs", "qa"] for i in range(len(lower) - 1))
    if qa and not item.is_dir():
        require(Path(lower[-1]).suffix in QA_TEXT_EXTENSIONS, "QA 目录只允许文字/结构化证据，禁止媒体或编码容器")
    return "/".join(lower), qa


def safe_archive(archive):
    seen = set()
    for item in archive.infolist():
        canonical, qa = safe_entry(item)
        require(canonical not in seen, "ZIP 包含大小写或规范化重复路径")
        seen.add(canonical)
        if qa and not item.is_dir():
            data = archive.read(item)
            require(b"\0" not in data and not ENCODED_MEDIA.search(data), "QA 文本含二进制或编码媒体")
            try:
                data.decode("utf-8-sig")
            except UnicodeDecodeError as error:
                raise RuntimeError("QA 证据必须是 UTF-8 文本，不能把媒体改后缀上传") from error
    require(seen, "ZIP 不得为空")


def verify_assets(root, pair):
    require({p.name for p in root.iterdir()} == ASSETS, "资产清单不完整或含额外文件")
    require(all(p.is_file() and not p.is_symlink() for p in root.iterdir()), "不接受链接或目录")
    manifest = json.loads((root / "release-pair.json").read_text(encoding="utf-8"))
    require(set(manifest) <= {"schema_version", "tag", "worldedit", "worldline", "ci", "acceptance", "build"}, "manifest 含未知字段")
    for key in ("schema_version", "tag", "worldedit", "worldline"):
        require(manifest.get(key) == pair[key], f"release-pair.json {key} 不符")
    lines = (root / "SHA256SUMS.txt").read_text(encoding="utf-8-sig").splitlines()
    hashes = {}
    for line in lines:
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9_.-]+)", line)
        require(match is not None and match[2] not in hashes, "无效/重复校验条目")
        hashes[match[2]] = match[1]
    require(set(hashes) == ASSETS - {"SHA256SUMS.txt"}, "校验清单必须覆盖每个其他资产")
    for name, digest in hashes.items():
        require(hashlib.sha256((root / name).read_bytes()).hexdigest() == digest, f"SHA256 不符：{name}")
    for name in ZIP_NAMES:
        with zipfile.ZipFile(root / name) as archive:
            safe_archive(archive)
            require(archive.testzip() is None, f"ZIP 损坏：{name}")
            names = archive.namelist()
            require(names and len(names) == len(set(names)), "空 ZIP 或重复 ZIP 条目")
            require(all(not n.startswith(("/", "\\")) and ".." not in n.replace("\\", "/").split("/")
                        and ":" not in n for n in names), "ZIP 路径越界")
            if name.endswith("source.zip"):
                repo_name = name.removesuffix("-source.zip")
                require(archive.comment.decode("ascii") == pair[repo_name]["sha"], "源码归档 commit 标记不符")
                require(all(n.startswith(repo_name + "/") for n in names), "源码 ZIP 根目录不符")
                cargo = tomllib.loads(archive.read(repo_name + "/Cargo.toml").decode("utf-8-sig"))
                version = cargo["package"]["version"] if repo_name == "worldedit" else cargo["workspace"]["package"]["version"]
                require(version == pair[repo_name]["version"], "源码版本不符")
                if repo_name == "worldedit":
                    compatibility = json.loads(archive.read("worldedit/compatibility.json"))
                    require(compatibility.get("worldline") == {"repository": CORE, "sha": pair["worldline"]["sha"]}, "源码固定配对不符")
                require(not any("/.git/" in n or "/target/" in n for n in names), "源码含私有/构建文件")
            if name == "worldedit-windows-x64.zip":
                require(all(n.startswith("windows/") for n in names), "Windows ZIP 根目录不符")
                require({"windows/worldedit.exe", "windows/wl.exe", "windows/wl-agent.exe"} <= set(names), "缺少 Windows 程序")
            if name == "worldedit-web.zip":
                require(all(n.startswith("web/") for n in names), "Web ZIP 根目录不符")
                require("web/index.html" in names and any(n.endswith(".wasm") for n in names)
                        and any(n.endswith(".js") for n in names), "缺少 Web 产物")
    return manifest


def release_body(tag):
    require(BRANCH.fullmatch("release/" + tag), "发行说明版本必须是规范tag")
    path = Path(__file__).resolve().parent.parent / "docs" / "releases" / (tag + ".md")
    if path.is_file():
        return path.read_text(encoding="utf-8")
    return "Windows/Web 与双仓源码。完整配对提交及 CI 见 release-pair.json；下载后核对 SHA256SUMS.txt。"


def publish(root):
    pair = gate()
    verify_assets(root, pair)
    tag, sha = pair["tag"], pair["worldedit"]["sha"]
    # Artifact validation can take time; recheck immediately before the first mutation.
    fresh = gate()
    require(all(fresh[k] == pair[k] for k in ("tag", "worldedit", "worldline")), "创建 draft 前配对发生变化")
    # No overwrite, clobber, release deletion, or tag-force operation is implemented.
    release_set(tag)
    api(f"repos/{EDITOR}/git/refs", "POST", {"ref": "refs/tags/" + tag, "sha": sha})
    release = api(f"repos/{EDITOR}/releases", "POST", {
        "tag_name": tag, "target_commitish": sha, "name": f"worldedit {tag}", "draft": True,
        "prerelease": False, "body": release_body(tag)})
    release_id = release["id"]
    require(release.get("draft") is True, "创建结果不是 draft，停止")
    release_set(tag, release_id, sha)
    gh("release", "upload", tag, *[str(root / n) for n in sorted(ASSETS)], "--repo", EDITOR)
    uploaded = api(f"repos/{EDITOR}/releases/{release_id}")
    require(uploaded.get("draft") is True and uploaded.get("tag_name") == tag
            and uploaded.get("target_commitish") == sha, "draft 元数据已变动")
    assets = pages(f"repos/{EDITOR}/releases/{release_id}/assets?per_page=100")
    require(len(assets) == len(ASSETS) and {a["name"] for a in assets} == ASSETS, "远端资产集合不符")
    for asset in assets:
        require(asset["state"] == "uploaded" and asset["size"] == (root / asset["name"]).stat().st_size,
                "远端资产未完成或大小不符")
    release_set(tag, release_id)
    with tempfile.TemporaryDirectory() as directory:
        downloaded = Path(directory)
        gh("release", "download", tag, "--repo", EDITOR, "--dir", directory)
        verify_assets(downloaded, pair)
        require(all((downloaded / n).read_bytes() == (root / n).read_bytes() for n in ASSETS),
                "上传后的逐字节校验失败")
    fresh = gate(existing=True)
    require(all(fresh[k] == pair[k] for k in ("tag", "worldedit", "worldline")), "公开前配对发生变化")
    final = api(f"repos/{EDITOR}/releases/{release_id}")
    require(final.get("draft") is True and final.get("tag_name") == tag
            and final.get("target_commitish") == sha, "公开前 draft 发生变化")
    require(sorted((a["id"], a["name"], a["size"], a.get("digest")) for a in final["assets"])
            == sorted((a["id"], a["name"], a["size"], a.get("digest")) for a in assets), "公开前资产发生变化")
    release_set(tag, release_id)
    api(f"repos/{EDITOR}/releases/{release_id}", "PATCH", {"draft": False, "make_latest": "true"})
    result = api(f"repos/{EDITOR}/releases/{release_id}")
    require(result.get("draft") is False and result.get("tag_name") == tag, "无法确认发行已公开，请人工检查")
    print(result["html_url"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["gate", "verify", "publish"])
    parser.add_argument("--directory", type=Path, default=Path("release-assets"))
    parser.add_argument("--pair", type=Path, default=Path("release-pair.json"))
    args = parser.parse_args()
    if args.command == "gate":
        pair = gate()
        args.pair.write_text(json.dumps(pair, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if os.environ.get("GITHUB_OUTPUT"):
            with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
                output.write(f"editor_sha={pair['worldedit']['sha']}\ncore_sha={pair['worldline']['sha']}\ntag={pair['tag']}\n")
    elif args.command == "verify":
        verify_assets(args.directory, json.loads(args.pair.read_text(encoding="utf-8")))
    else:
        publish(args.directory)


if __name__ == "__main__":
    main()
