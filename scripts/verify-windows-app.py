"""Cross-platform, dependency-free checks against the actual Windows PE binaries.

This is static artifact verification, not evidence of a Windows GUI launch.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


class VerificationError(ValueError):
    """A binary does not meet the desktop application identity contract."""


def require(condition, message):
    if not condition:
        raise VerificationError(message)


def chunk(data, offset, length, label):
    require(offset >= 0 and length >= 0 and offset + length <= len(data),
            f"truncated or invalid {label}")
    return data[offset:offset + length]


def unpack(data, offset, fmt, label):
    return struct.unpack(fmt, chunk(data, offset, struct.calcsize(fmt), label))


class PortableExecutable:
    def __init__(self, data):
        self.data = data
        require(chunk(data, 0, 2, "DOS header") == b"MZ", "missing DOS MZ signature")
        pe = unpack(data, 0x3C, "<I", "PE pointer")[0]
        require(chunk(data, pe, 4, "PE signature") == b"PE\0\0", "missing PE signature")
        coff = unpack(data, pe + 4, "<HHIIIHH", "COFF header")
        count, optional_size = coff[1], coff[5]
        require(coff[6] & 0x0002 and not coff[6] & 0x2000,
                "expected an executable image, not a DLL or object file")
        require(0 < count <= 96, "invalid PE section count")
        optional = chunk(data, pe + 24, optional_size, "optional header")
        magic = unpack(optional, 0, "<H", "optional header magic")[0]
        require(magic in (0x10B, 0x20B), "expected a PE32 or PE32+ executable")
        self.subsystem = unpack(optional, 68, "<H", "subsystem")[0]
        directory_count_offset, directories = (92, 96) if magic == 0x10B else (108, 112)
        directory_count = unpack(optional, directory_count_offset, "<I", "directory count")[0]
        self.resource_rva, self.resource_size = (0, 0)
        if directory_count > 2:
            self.resource_rva, self.resource_size = unpack(
                optional, directories + 16, "<II", "resource directory entry")
        self.sections = []
        for index in range(count):
            header = chunk(data, pe + 24 + optional_size + index * 40, 40, "section header")
            virtual_size, address, raw_size, raw_offset = unpack(header, 8, "<IIII", "section")
            chunk(data, raw_offset, raw_size, "section data")
            self.sections.append((address, virtual_size, raw_offset, raw_size))

    def at_rva(self, address, size):
        matches = []
        for start, virtual_size, offset, raw_size in self.sections:
            relative = address - start
            if 0 <= relative < max(virtual_size, raw_size) and relative + size <= raw_size:
                matches.append(offset + relative)
        require(len(matches) == 1, f"unmapped or ambiguous resource RVA 0x{address:x}")
        return chunk(self.data, matches[0], size, "resource payload")

    def resources(self):
        if self.resource_rva == self.resource_size == 0:
            return {}
        require(self.resource_rva > 0 and self.resource_size > 0, "incomplete resource directory")
        directory = self.at_rva(self.resource_rva, self.resource_size)
        resources = {}

        def identifier(value):
            if not value & 0x80000000:
                return value
            offset = value & 0x7FFFFFFF
            length = unpack(directory, offset, "<H", "resource name length")[0]
            try:
                return chunk(directory, offset + 2, length * 2, "resource name").decode("utf-16-le")
            except UnicodeDecodeError as error:
                raise VerificationError("invalid resource name") from error

        def visit(offset, path, ancestors):
            require(offset not in ancestors, "cycle in resource directory")
            require(len(path) < 3, "resource directory exceeds type/name/language depth")
            header = unpack(directory, offset, "<IIHHHH", "resource directory header")
            count = header[4] + header[5]
            entries = chunk(directory, offset + 16, count * 8, "resource directory entries")
            seen = set()
            for index in range(count):
                name, target = unpack(entries, index * 8, "<II", "resource entry")
                name = identifier(name)
                require(name not in seen, "duplicate name in resource directory")
                seen.add(name)
                child = path + (name,)
                if target & 0x80000000:
                    visit(target & 0x7FFFFFFF, child, ancestors | {offset})
                else:
                    require(len(child) == 3, "resource leaf must have type, name and language")
                    address, size, _, reserved = unpack(directory, target, "<IIII", "resource data entry")
                    require(reserved == 0 and size > 0, "invalid resource data entry")
                    resources[child] = self.at_rva(address, size)

        visit(0, (), set())
        return resources


def ico_images(icon):
    reserved, kind, count = unpack(icon, 0, "<HHH", "ICO header")
    require(reserved == 0 and kind == 1 and count > 0, "expected a nonempty ICO file")
    chunk(icon, 6, count * 16, "ICO directory")
    images = []
    for index in range(count):
        entry = chunk(icon, 6 + index * 16, 16, "ICO entry")
        width, height, _, reserved, _, _, size, offset = unpack(entry, 0, "<BBBBHHII", "ICO entry")
        require(reserved == 0 and size > 0 and offset >= 6 + count * 16, "invalid ICO entry")
        width, height = width or 256, height or 256
        require(width == height, "application icon sizes must be square")
        images.append((entry[:12], chunk(icon, offset, size, "ICO image"), width))
    require(sorted(image[2] for image in images) == [16, 24, 32, 48, 64, 128, 256],
            "bundled application icon must contain all seven branded sizes")
    return images


def verify_icon_resources(pe, icon):
    expected = ico_images(icon)
    resources = pe.resources()
    groups = [(key, data) for key, data in resources.items() if key[0] == 14]
    icons = {key: data for key, data in resources.items() if key[0] == 3}
    require(len(groups) == 1, f"expected one RT_GROUP_ICON, found {len(groups)}")
    require(len(icons) == len(expected), f"expected {len(expected)} RT_ICON images, found {len(icons)}")
    group_key, group = groups[0]
    require(group[:6] == icon[:6], "RT_GROUP_ICON header does not match bundled ICO")
    require(len(group) == 6 + 14 * len(expected), "RT_GROUP_ICON has incorrect size")
    used_ids = set()
    for index, (header, payload, size) in enumerate(expected):
        entry = chunk(group, 6 + index * 14, 14, "group icon entry")
        require(entry[:12] == header, f"{size}px group metadata differs from bundled ICO")
        resource_id = unpack(entry, 12, "<H", "icon resource ID")[0]
        require(resource_id not in used_ids, "group icon reuses an image resource ID")
        used_ids.add(resource_id)
        actual = icons.get((3, resource_id, group_key[2]))
        require(actual == payload, f"{size}px RT_ICON bytes differ from bundled ICO or are missing")
    return {
        "icon_group_count": len(groups),
        "icon_image_count": len(icons),
        "icon_sizes": sorted(image[2] for image in expected),
    }


def verify(exe, icon, cli=None):
    exe_data, icon_data = Path(exe).read_bytes(), Path(icon).read_bytes()
    pe = PortableExecutable(exe_data)
    require(pe.subsystem == 2, f"worldedit must use WINDOWS_GUI (2), found subsystem {pe.subsystem}")
    result = {
        "ok": True,
        "verification": "static-pe",
        "subsystem": pe.subsystem,
        "subsystem_name": "WINDOWS_GUI",
        **verify_icon_resources(pe, icon_data),
        "icon_sha256": hashlib.sha256(icon_data).hexdigest(),
        "exe_sha256": hashlib.sha256(exe_data).hexdigest(),
    }
    if cli is not None:
        cli_data = Path(cli).read_bytes()
        cli_pe = PortableExecutable(cli_data)
        require(cli_pe.subsystem == 3, f"wl must retain WINDOWS_CUI (3), found subsystem {cli_pe.subsystem}")
        result.update(cli_subsystem=cli_pe.subsystem, cli_subsystem_name="WINDOWS_CUI",
                      cli_sha256=hashlib.sha256(cli_data).hexdigest())
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path, help="actual worldedit.exe, including an extracted release binary")
    parser.add_argument("--icon", type=Path, required=True, help="the unchanged bundled worldedit.ico")
    parser.add_argument("--cli", type=Path, help="also verify that the companion wl.exe remains a console CLI")
    arguments = parser.parse_args()
    try:
        print(json.dumps(verify(arguments.exe, arguments.icon, arguments.cli), ensure_ascii=False, sort_keys=True))
    except (OSError, VerificationError) as error:
        print(f"Windows application verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
