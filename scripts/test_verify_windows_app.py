"""Synthetic PE tests for static GUI/CLI and byte-exact icon resource checks."""

import importlib.util
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("verify-windows-app.py")
SPEC = importlib.util.spec_from_file_location("verify_windows_app", SCRIPT)
app = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(app)
ICON = SCRIPT.parent.parent / "assets/worldedit.ico"


def resource_fixture(icon, mutate=None):
    images = app.ico_images(icon)
    group = icon[:6] + b"".join(header + struct.pack("<H", index + 1)
                                for index, (header, _, _) in enumerate(images))
    tree = {3: {index + 1: {0: payload} for index, (_, payload, _) in enumerate(images)},
            14: {1: {0: group}}}
    if mutate is not None:
        mutate(tree)
    data = bytearray()

    def reserve(count):
        offset = len(data)
        data.extend(b"\0" * count)
        return offset

    def write_directory(children):
        offset = reserve(16 + len(children) * 8)
        struct.pack_into("<HH", data, offset + 12, 0, len(children))
        for index, (name, child) in enumerate(sorted(children.items())):
            if isinstance(child, dict):
                target = write_directory(child) | 0x80000000
            else:
                target = reserve(16)
                payload = reserve(len(child))
                data[payload:payload + len(child)] = child
                struct.pack_into("<IIII", data, target, 0x1000 + payload, len(child), 0, 0)
                while len(data) % 4:
                    data.append(0)
            struct.pack_into("<II", data, offset + 16 + index * 8, name, target)
        return offset

    write_directory(tree)
    return data


def pe_fixture(resource=b"", subsystem=2, pe32=False):
    optional_size = 224 if pe32 else 240
    headers = bytearray(512)
    headers[:2] = b"MZ"
    struct.pack_into("<I", headers, 0x3C, 0x80)
    headers[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", headers, 0x84, 0x14C if pe32 else 0x8664, 1, 0, 0, 0, optional_size, 2)
    optional = 0x98
    struct.pack_into("<H", headers, optional, 0x10B if pe32 else 0x20B)
    struct.pack_into("<H", headers, optional + 68, subsystem)
    struct.pack_into("<I", headers, optional + (92 if pe32 else 108), 16)
    if resource:
        struct.pack_into("<II", headers, optional + (96 if pe32 else 112) + 16, 0x1000, len(resource))
    section = optional + optional_size
    headers[section:section + 8] = b".rsrc\0\0\0"
    struct.pack_into("<IIII", headers, section + 8, len(resource), 0x1000, len(resource), 512)
    return headers + resource


class WindowsAppTests(unittest.TestCase):
    def setUp(self):
        self.icon = ICON.read_bytes()

    def test_existing_brand_contains_all_seven_sizes(self):
        self.assertEqual([image[2] for image in app.ico_images(self.icon)], [16, 24, 32, 48, 64, 128, 256])

    def test_accepts_byte_exact_pe32_and_pe32_plus_icons(self):
        for pe32 in (False, True):
            with self.subTest(pe32=pe32):
                pe = app.PortableExecutable(pe_fixture(resource_fixture(self.icon), pe32=pe32))
                result = app.verify_icon_resources(pe, self.icon)
                self.assertEqual(result["icon_group_count"], 1)
                self.assertEqual(result["icon_image_count"], 7)
                self.assertEqual(result["icon_sizes"], [16, 24, 32, 48, 64, 128, 256])

    def test_rejects_corrupt_payload_even_with_correct_resource_counts(self):
        def corrupt(tree):
            original = tree[3][7][0]
            tree[3][7][0] = original[:-1] + bytes([original[-1] ^ 1])
        pe = app.PortableExecutable(pe_fixture(resource_fixture(self.icon, corrupt)))
        with self.assertRaisesRegex(app.VerificationError, "256px RT_ICON bytes"):
            app.verify_icon_resources(pe, self.icon)

    def test_rejects_missing_extra_or_wrong_language_icon_resources(self):
        def missing(tree):
            del tree[3][3]
        def extra(tree):
            tree[3][8] = {0: b"not branded"}
        def wrong_language(tree):
            tree[3][4] = {1033: tree[3][4][0]}
        def extra_group(tree):
            tree[14][2] = tree[14][1]
        for mutation in (missing, extra, wrong_language, extra_group):
            with self.subTest(mutation=mutation.__name__), self.assertRaises(app.VerificationError):
                app.verify_icon_resources(app.PortableExecutable(pe_fixture(resource_fixture(self.icon, mutation))), self.icon)

    def test_rejects_wrong_group_metadata_and_reused_resource_ids(self):
        def metadata(tree):
            group = bytearray(tree[14][1][0])
            group[6] = 17
            tree[14][1][0] = bytes(group)
        def duplicate_id(tree):
            group = bytearray(tree[14][1][0])
            struct.pack_into("<H", group, 6 + 14 + 12, 1)
            tree[14][1][0] = bytes(group)
        for mutation in (metadata, duplicate_id):
            with self.subTest(mutation=mutation.__name__), self.assertRaises(app.VerificationError):
                app.verify_icon_resources(app.PortableExecutable(pe_fixture(resource_fixture(self.icon, mutation))), self.icon)

    def test_rejects_missing_icons_and_invalid_resource_pointers(self):
        with self.assertRaisesRegex(app.VerificationError, "RT_GROUP_ICON"):
            app.verify_icon_resources(app.PortableExecutable(pe_fixture()), self.icon)
        bad = resource_fixture(self.icon)
        struct.pack_into("<I", bad, 20, 0x80000000)
        with self.assertRaisesRegex(app.VerificationError, "cycle"):
            app.PortableExecutable(pe_fixture(bad)).resources()
        bad = resource_fixture(self.icon)
        struct.pack_into("<I", bad, 20, 0xFFFFFFF0)
        with self.assertRaises(app.VerificationError):
            app.PortableExecutable(pe_fixture(bad)).resources()

    def test_rejects_truncated_or_non_pe_binaries_and_bad_ico(self):
        complete = pe_fixture(resource_fixture(self.icon))
        for data in (b"", b"not an exe", complete[:0x80], complete[:250], complete[:-1]):
            with self.subTest(length=len(data)), self.assertRaises(app.VerificationError):
                app.PortableExecutable(data)
        for data in (b"", b"bad icon", self.icon[:6], self.icon[:-1]):
            with self.subTest(length=len(data)), self.assertRaises(app.VerificationError):
                app.ico_images(data)

    def test_rejects_dll_and_object_masquerading_as_an_application(self):
        for flags in (0, 0x2002):
            binary = pe_fixture(resource_fixture(self.icon))
            struct.pack_into("<H", binary, 0x84 + 18, flags)
            with self.subTest(flags=flags), self.assertRaisesRegex(app.VerificationError, "executable image"):
                app.PortableExecutable(binary)

    def test_actual_file_interface_reports_static_verification_and_preserves_cli(self):
        with tempfile.TemporaryDirectory() as directory:
            exe, cli = Path(directory) / "worldedit.exe", Path(directory) / "wl.exe"
            exe.write_bytes(pe_fixture(resource_fixture(self.icon)))
            cli.write_bytes(pe_fixture(subsystem=3))
            result = app.verify(exe, ICON, cli)
            self.assertTrue(result["ok"])
            self.assertEqual(result["verification"], "static-pe")
            self.assertEqual(result["subsystem"], 2)
            self.assertEqual(result["cli_subsystem"], 3)
            self.assertEqual(len(result["exe_sha256"]), 64)
            cli.write_bytes(pe_fixture(subsystem=2))
            with self.assertRaisesRegex(app.VerificationError, "wl must retain"):
                app.verify(exe, ICON, cli)
            exe.write_bytes(pe_fixture(resource_fixture(self.icon), subsystem=3))
            with self.assertRaisesRegex(app.VerificationError, "worldedit must use"):
                app.verify(exe, ICON)

    def test_command_line_failure_is_nonzero_and_does_not_claim_success(self):
        result = subprocess.run([sys.executable, str(SCRIPT), str(ICON), "--icon", str(ICON)],
                                capture_output=True, text=True, encoding="utf-8", check=False)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("verification failed", result.stderr)


if __name__ == "__main__":
    unittest.main()
