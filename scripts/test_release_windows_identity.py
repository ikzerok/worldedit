"""最终 Windows ZIP 的资源核验不得退回构建目录或跳过安全筛选。"""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile


SPEC = importlib.util.spec_from_file_location("release_build", Path(__file__).with_name("release-build.py"))
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class PackagedWindowsIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "windows.zip"
        with zipfile.ZipFile(self.archive, "w") as archive:
            for name in ("worldedit.exe", "worldedit.ico", "wl.exe"):
                archive.writestr("windows/" + name, ("packaged " + name).encode())

    def test_verifies_bytes_from_final_zip_and_cleans_temporary_extraction(self):
        paths = []
        expected = {"ok": True, "subsystem": 2, "cli_subsystem": 3}

        def verify(executable, icon, cli):
            for path in (executable, icon, cli):
                self.assertEqual(path.read_bytes(), ("packaged " + path.name).encode())
                self.assertFalse(path.is_relative_to(self.root))
                paths.append(path)
            return expected

        with patch.object(builder, "windows_identity", side_effect=verify):
            self.assertEqual(builder.packaged_windows_identity(self.archive), expected)
        self.assertTrue(paths)
        self.assertTrue(all(not path.exists() for path in paths))
        self.assertEqual(list(self.root.iterdir()), [self.archive])

    def test_verifier_failure_propagates_and_does_not_leave_extracted_files(self):
        with patch.object(builder, "windows_identity", side_effect=RuntimeError("bad PE")):
            with self.assertRaisesRegex(RuntimeError, "bad PE"):
                builder.packaged_windows_identity(self.archive)
        self.assertEqual(list(self.root.iterdir()), [self.archive])

    def test_unsafe_zip_is_rejected_before_verifier(self):
        with zipfile.ZipFile(self.archive, "a") as archive:
            archive.writestr("../unexpected.txt", "invalid")
        with patch.object(builder, "windows_identity") as verify:
            with self.assertRaises(RuntimeError):
                builder.packaged_windows_identity(self.archive)
            verify.assert_not_called()


if __name__ == "__main__":
    unittest.main()
