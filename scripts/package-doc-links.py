#!/usr/bin/env python3
"""只重映射 Windows 发行副本的 Markdown 链接；不修改源码或源码归档。"""
import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
from urllib.parse import quote, unquote, urlsplit, urlunsplit


MARKER = "worldedit-package-doc-links:v1"
ONLINE_NOTICE = ("> 发行包说明：本页保留的 GitHub 源码链接需要联网，对应源码或历史材料未装入此应用包；"
                 "本次重映射的源码链接使用发行配对的固定提交。")
ALIASES = {
    "assets/worldedit.ico": "worldedit.ico",
    "assets/fonts/OFL.txt": "FONT-LICENSE.txt",
    "assets/licenses/resvg-MIT.txt": "RESVG-LICENSE.txt",
    "assets/licenses/self-cell-APACHE.txt": "SELF-CELL-LICENSE.txt",
}


def omitted_source(repo, relative):
    """Git export-ignore 也会移除这些目标；仅允许既有明确排除范围。"""
    parts = relative.parts
    return (repo == "worldline" and (parts[:1] == ("examples",) or parts[:2] == ("spec", "examples"))
            or repo == "worldedit" and relative.as_posix()
            == "docs/design/editor-system/20260926/eds10-visual-sample.html")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular_path(path, root):
    """拒绝链接/重解析点及特殊文件，不能借文档目标访问目录外内容。"""
    require(path.is_relative_to(root), "文档路径越过许可根目录")
    current = path
    while True:
        if current.exists() or current.is_symlink():
            meta = current.lstat()
            attributes = getattr(meta, "st_file_attributes", 0)
            require(not current.is_symlink()
                    and not attributes & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400),
                    "文档路径不能包含链接或重解析点")
            require(stat.S_ISDIR(meta.st_mode) or stat.S_ISREG(meta.st_mode), "文档路径包含特殊文件")
        if current == root:
            break
        current = current.parent


def without_code(body):
    """仅屏蔽 fenced/indented/inline code，保留所有字符位置和换行。"""
    chars = list(body)
    offset = 0
    fence = None
    for line in body.splitlines(keepends=True):
        match = re.match(r" {0,3}(`{3,}|~{3,})", line)
        masked = fence is not None or line.startswith(("    ", "\t"))
        if match:
            token = match.group(1)
            if fence is None:
                fence = token
                masked = True
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = None
        if masked:
            for index in range(offset, offset + len(line)):
                if chars[index] not in "\r\n":
                    chars[index] = " "
        offset += len(line)
    visible = "".join(chars)
    for match in re.finditer(r"(`+)([^\n]*?)\1(?!`)", visible):
        for index in range(match.start(), match.end()):
            chars[index] = " "
    return "".join(chars)


@dataclass(frozen=True)
class Link:
    start: int
    end: int
    image_start: int | None = None


def destination(text, start):
    while start < len(text) and text[start].isspace():
        start += 1
    if start >= len(text):
        return None
    if text[start] == "<":
        end = text.find(">", start + 1)
        return (start + 1, end, end + 1) if end >= 0 else None
    end = start
    depth = 0
    while end < len(text):
        char = text[end]
        if char == "\\" and end + 1 < len(text):
            end += 2
            continue
        if char.isspace() or char == ")" and depth == 0:
            break
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        end += 1
    return start, end, end


def markdown_links(body):
    visible = without_code(body)
    links = []
    # Reference definitions keep their labels and title text unchanged.
    for match in re.finditer(r"^ {0,3}\[[^\]\n]+\]:[ \t]*", visible, re.MULTILINE):
        value = destination(visible, match.end())
        if value and value[0] < value[1]:
            links.append(Link(value[0], value[1]))
    index = 0
    while index < len(visible):
        if visible[index] != "[" or index > 0 and visible[index - 1] == "\\":
            index += 1
            continue
        closing = index + 1
        depth = 1
        while closing < len(visible) and depth:
            if visible[closing] == "\\":
                closing += 2
                continue
            if visible[closing] == "[":
                depth += 1
            elif visible[closing] == "]":
                depth -= 1
            closing += 1
        if depth or closing >= len(visible) or visible[closing] != "(":
            index += 1
            continue
        value = destination(visible, closing + 1)
        if not value:
            index = closing + 1
            continue
        start, end, following = value
        # A destination may be followed by a quoted title; require a real link close.
        tail = visible[following:]
        close = re.match(r'''\s*(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\))?\s*\)''', tail)
        if close:
            image = index - 1 if index > 0 and visible[index - 1] == "!" else None
            links.append(Link(start, end, image))
            index = following + close.end()
        else:
            index = closing + 1
    return sorted(set(links), key=lambda link: link.start)


def inject_header(body, marker, online):
    newline = "\r\n" if "\r\n" in body else "\n"
    prefix = f"<!-- {marker} -->" + newline
    if online:
        prefix += ONLINE_NOTICE + newline + newline
    position = 1 if body.startswith("\ufeff") else 0
    frontmatter = re.match(r"---\r?\n.*?^---[ \t]*\r?\n", body[position:], re.MULTILINE | re.DOTALL)
    if frontmatter:
        position += frontmatter.end()
    return body[:position] + prefix + body[position:]


class Remapper:
    def __init__(self, desktop, editor, core, editor_sha, core_sha):
        self.desktop = Path(os.path.abspath(desktop))
        self.roots = {"worldedit": Path(os.path.abspath(editor)), "worldline": Path(os.path.abspath(core))}
        self.shas = {"worldedit": editor_sha.lower(), "worldline": core_sha.lower()}
        for sha in self.shas.values():
            require(re.fullmatch(r"[0-9a-f]{40}", sha) is not None, "必须提供两仓完整40位提交SHA")
        for root in [self.desktop, *self.roots.values()]:
            require(root.is_dir(), "源码与发行目录必须已经存在")
            regular_path(root, Path(root.anchor))
        for root in self.roots.values():
            require(not self.desktop.is_relative_to(root) and not root.is_relative_to(self.desktop),
                    "发行目录与源码目录不得互相包含")
        require(not self.roots["worldedit"].is_relative_to(self.roots["worldline"])
                and not self.roots["worldline"].is_relative_to(self.roots["worldedit"]),
                "两仓源码根目录必须独立")
        self.marker = f"{MARKER} editor={editor_sha.lower()} worldline={core_sha.lower()}"

    def source_for(self, staged):
        relative = staged.relative_to(self.desktop)
        repo = "worldline" if relative.parts[0] == "worldline" else "worldedit"
        if repo == "worldline":
            relative = Path(*relative.parts[1:])
        source = self.roots[repo] / relative
        require(source.is_file(), f"发行Markdown没有源码对应文件：{staged.relative_to(self.desktop)}")
        regular_path(source, self.roots[repo])
        require(not os.path.samefile(source, staged), "发行文档不能是源码的硬链接")
        return source

    def identity(self, path):
        path = Path(os.path.abspath(path))
        for repo, root in self.roots.items():
            if path.is_relative_to(root):
                regular_path(path, root)
                relative = path.relative_to(root)
                require(path.exists() or omitted_source(repo, relative),
                        f"源码文档链接目标缺失：{repo}/{relative}")
                return repo, relative
        raise ValueError("相对链接越过两仓源码边界")

    def staged_for(self, repo, relative):
        if repo == "worldline":
            target = self.desktop / "worldline" / relative
        else:
            target = self.desktop / ALIASES.get(relative.as_posix(), relative.as_posix())
        regular_path(target, self.desktop)
        return target if target.exists() else None

    def rewrite(self, url, source, staged):
        plain = re.sub(r"\\([\\`()<> ])", r"\1", url)
        parsed = urlsplit(plain)
        identity = None
        source_kind = None
        if parsed.scheme or parsed.netloc:
            require(parsed.scheme != "file", "发行文档不得保留本机file链接")
            if parsed.netloc == "github.com":
                match = re.fullmatch(r"/ikzerok/(worldedit|worldline)/(blob|tree)/([^/]+)/(.+)", parsed.path)
                if match:
                    repo, source_kind, ref, path = match.groups()
                    if ref in {"main", self.shas[repo]}:
                        identity = self.identity(self.roots[repo] / unquote(path))
                    else:
                        return url, "online-existing"
            if identity is None:
                return url, "external"
        elif not parsed.path:
            return url, "same-document"
        else:
            require(not parsed.path.startswith(("/", "\\")) and "\\" not in plain,
                    "发行文档链接必须是标准相对URL")
            identity = self.identity(source.parent / unquote(parsed.path))
        repo, relative = identity
        target = self.staged_for(repo, relative)
        if target is not None:
            path = quote(Path(os.path.relpath(target, staged.parent)).as_posix(), safe="/.-_")
            return urlunsplit(("", "", path, parsed.query, parsed.fragment)), "local"
        source_target = self.roots[repo] / relative
        directory = source_target.is_dir() or (not source_target.exists()
                    and (parsed.path.endswith("/") or relative.as_posix() in {"examples", "spec/examples"}))
        kind = source_kind or ("tree" if directory else "blob")
        path = f"/ikzerok/{repo}/{kind}/{self.shas[repo]}/" + quote(relative.as_posix(), safe="/.-_")
        return urlunsplit(("https", "github.com", path, parsed.query, parsed.fragment)), "online-pinned"

    def validate_output(self, body, staged):
        """每次检查实际发行链接；已有标记不能掩盖后来缺失的文件。"""
        for link in markdown_links(body):
            url = urlsplit(body[link.start:link.end])
            require(url.scheme != "file", "发行文档不得保留本机file链接")
            if url.scheme or url.netloc or not url.path:
                continue
            path = unquote(url.path)
            require(not path.startswith(("/", "\\")) and "\\" not in path,
                    "发行文档链接必须是标准相对URL")
            target = Path(os.path.abspath(staged.parent / path))
            regular_path(target, self.desktop)
            require(target.exists(), f"发行文档链接目标缺失：{staged.relative_to(self.desktop)} -> {url.path}")

    def plan(self):
        pending, records = [], []
        skipped = 0
        files = []
        for path in sorted(self.desktop.rglob("*")):
            regular_path(path, self.desktop)
            if path.suffix.casefold() == ".md":
                files.append(path)
        for staged in files:
            regular_path(staged, self.desktop)
            require(staged.is_file(), "Markdown目标不是普通文件")
            source = self.source_for(staged)
            body = staged.read_bytes().decode("utf-8")
            if MARKER in body:
                require(f"<!-- {self.marker} -->" in body, "发行文档已由不同提交重映射，须重新复制源码")
                self.validate_output(body, staged)
                skipped += 1
                continue
            edits, online = [], False
            for link in markdown_links(body):
                old = body[link.start:link.end]
                new, kind = self.rewrite(old, source, staged)
                online |= kind.startswith("online")
                if new != old:
                    edits.append((link.start, link.end, new))
                    records.append({"file": staged.relative_to(self.desktop).as_posix(),
                                    "line": body.count("\n", 0, link.start) + 1,
                                    "before": old, "after": new, "kind": kind})
                if link.image_start is not None and kind.startswith("online"):
                    # A GitHub blob page is a readable source link, not an image response.
                    edits.append((link.image_start, link.image_start + 1, ""))
            for start, end, value in sorted(edits, reverse=True):
                body = body[:start] + value + body[end:]
            self.validate_output(body, staged)
            pending.append((staged, inject_header(body, self.marker, online).encode("utf-8")))
        report = {"markdown_files": len(files), "rewritten_files": len(pending), "already_mapped": skipped,
                  "local_rewrites": sum(row["kind"] == "local" for row in records),
                  "online_rewrites": sum(row["kind"] == "online-pinned" for row in records),
                  "editor_sha": self.shas["worldedit"], "worldline_sha": self.shas["worldline"],
                  "changes": records}
        return pending, report


def remap(desktop, editor, core, editor_sha, core_sha, check=False):
    mapper = Remapper(desktop, editor, core, editor_sha, core_sha)
    pending, report = mapper.plan()  # Validate every document before the first write.
    if not check:
        for path, data in pending:
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(prefix=".doc-links-", dir=path.parent, delete=False) as output:
                    temporary = Path(output.name)
                    output.write(data)
                temporary.chmod(stat.S_IMODE(path.stat().st_mode))
                os.replace(temporary, path)
            finally:
                if temporary is not None and temporary.exists():
                    temporary.unlink()
    return {"ok": True, "mode": "check" if check else "apply", **report}


def main():
    # Keep the host encoding; escape undisplayable help/diagnostic characters.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--desktop-root", type=Path, required=True)
    parser.add_argument("--editor-source", type=Path, required=True)
    parser.add_argument("--worldline-source", type=Path, required=True)
    parser.add_argument("--editor-sha", required=True)
    parser.add_argument("--worldline-sha", required=True)
    parser.add_argument("--check", action="store_true", help="只输出完整重映射计划，不改文件")
    args = parser.parse_args()
    result = remap(args.desktop_root, args.editor_source, args.worldline_source,
                   args.editor_sha, args.worldline_sha, args.check)
    # JSON's Unicode escapes round-trip even non-BMP characters on legacy pipes.
    print(json.dumps(result, ensure_ascii=True, indent=2))


if __name__ == "__main__":
    main()
