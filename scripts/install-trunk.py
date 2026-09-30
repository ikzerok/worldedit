"""Windows CI 安装固定官方 Trunk；下一步用 --verify-path 核对 PATH 和版本。

维护时一并复核官方版本、ZIP/EXE 哈希和大小上限，不从网络更新信任值。
来源：https://github.com/trunk-rs/trunk/releases/tag/v0.21.14
上游预编译供应链不等价于使用本仓库固定 Rust 工具链自行编译。
"""
import argparse
import hashlib
import io
import os
from pathlib import Path
import platform
import shutil
import stat
import subprocess
import urllib.parse
import urllib.request
import zipfile

VERSION = "0.21.14"
URL = ("https://github.com/trunk-rs/trunk/releases/download/v0.21.14/"
       "trunk-x86_64-pc-windows-msvc.zip")
ZIP_SHA256 = "cd6ac15b9daff0365e5695036791ef2ce3c63f61c014f5a8c532363266e4569c"
EXE_SHA256 = "209f218e2c01516ef4e59b97ab73b879ff2180c73f4187fa152b15532e8eecae"
MAX_ZIP_BYTES = 8 * 1024 * 1024
MAX_EXE_BYTES = 32 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def check_url(url):
    parsed = urllib.parse.urlsplit(url)
    require(parsed.scheme == "https" and parsed.hostname in {
        "github.com", "release-assets.githubusercontent.com"
    } and parsed.port in {None, 443} and parsed.username is None and parsed.password is None,
        "Trunk 下载只允许官方 GitHub HTTPS 地址")


class OfficialRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        check_url(new_url)
        return super().redirect_request(request, response, code, message, headers, new_url)


def read_bounded(source, limit):
    data = source.read(limit + 1)
    require(0 < len(data) <= limit, "Trunk 文件为空或超过大小上限")
    return data


def download():
    check_url(URL)
    # 不读取 token、netrc 或代理凭据；仅公开 GET，不继承认证 handler。
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), OfficialRedirect())
    request = urllib.request.Request(URL, headers={"User-Agent": "worldedit-trunk-bootstrap"})
    with opener.open(request, timeout=60) as response:
        check_url(response.geturl())
        require(response.status == 200, "Trunk 下载未返回完整文件")
        length = response.headers.get("Content-Length")
        require(length is None or 0 < int(length) <= MAX_ZIP_BYTES, "Trunk 下载声明超过大小上限")
        return read_bounded(response, MAX_ZIP_BYTES)


def unpack_verified(data):
    require(0 < len(data) <= MAX_ZIP_BYTES, "Trunk ZIP 超过大小上限")
    require(hashlib.sha256(data).hexdigest() == ZIP_SHA256, "Trunk ZIP SHA256 不匹配")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        require(len(entries) == 1, "Trunk ZIP 必须只有一个条目")
        entry = entries[0]
        require(entry.filename == entry.orig_filename == "trunk.exe", "Trunk ZIP 路径不符")
        require(stat.S_IFMT(entry.external_attr >> 16) in {0, stat.S_IFREG}
                and not entry.external_attr & (0x10 | 0x400), "Trunk ZIP 不能包含链接或目录")
        require(not entry.flag_bits & 1 and entry.compress_type in {
            zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED
        }, "Trunk ZIP 压缩方式不符或已加密")
        require(0 < entry.file_size <= MAX_EXE_BYTES, "Trunk EXE 超过大小上限")
        with archive.open(entry) as source:
            binary = read_bounded(source, MAX_EXE_BYTES)
        require(len(binary) == entry.file_size, "Trunk EXE 长度不符")
    require(hashlib.sha256(binary).hexdigest() == EXE_SHA256, "Trunk EXE SHA256 不匹配")
    return binary


def installed_path():
    require(platform.system() == "Windows" and platform.machine().lower() in {"amd64", "x86_64"},
            "固定 Trunk 仅支持 Windows x64 runner")
    root = Path(os.environ["RUNNER_TEMP"]).resolve(strict=True)
    require(root.is_dir(), "RUNNER_TEMP 必须是已有目录")
    return root / ("worldedit-trunk-" + VERSION) / "trunk.exe"


def verify_executable(executable):
    require(not executable.is_symlink() and not executable.parent.is_symlink(), "Trunk 路径不能是链接")
    with executable.open("rb") as source:
        binary = read_bounded(source, MAX_EXE_BYTES)
    require(hashlib.sha256(binary).hexdigest() == EXE_SHA256, "已安装 Trunk SHA256 不匹配")
    result = subprocess.run([str(executable), "--version"], check=True, capture_output=True,
                            text=True, encoding="utf-8", timeout=30)
    require(result.stdout in {"trunk " + VERSION, "trunk " + VERSION + "\n"}
            and result.stderr == "", "Trunk 版本输出不符")


def install():
    executable = installed_path()
    path_file = Path(os.environ["GITHUB_PATH"])
    require(path_file.is_absolute() and path_file.is_file() and not path_file.is_symlink(),
            "GITHUB_PATH 必须是已有 runner 环境文件")
    binary = unpack_verified(download())
    executable.parent.mkdir()  # 已有目录直接失败，不复用旧文件或缓存。
    try:
        with executable.open("xb") as destination:
            destination.write(binary)
        verify_executable(executable)
        with path_file.open("a", encoding="utf-8") as destination:
            destination.write(str(executable.parent) + "\n")
    except BaseException:
        shutil.rmtree(executable.parent)
        raise
    print(f"Verified official Trunk {VERSION}; verify PATH in the next step before use")


def verify_path():
    executable = installed_path()
    actual = shutil.which("trunk")
    require(actual is not None and Path(actual).resolve(strict=True) == executable,
            "PATH 未命中本次安装的固定 Trunk")
    verify_executable(executable)
    print(f"Verified Trunk PATH, SHA256 and version: {str(executable)!a}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-path", action="store_true", help="不联网，核对下一步 PATH 命中及版本")
    args = parser.parse_args()
    if args.verify_path:
        verify_path()
    else:
        install()
