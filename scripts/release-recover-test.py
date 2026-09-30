import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("recover", Path(__file__).with_name("release-recover.py"))
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)


class RecoveryTests(unittest.TestCase):
    def draft(self):
        return {"id": r.DRAFT, "tag_name": r.TAG, "target_commitish": r.SOURCE,
                "draft": True, "author": {"login": "github-actions[bot]"}, "assets": []}

    def test_only_exact_empty_owned_draft(self):
        r.validate_draft(self.draft(), empty=True)
        for field, bad in [("id", 1), ("tag_name", "v0.3.2"), ("target_commitish", "a" * 40),
                           ("draft", False), ("author", {"login": "other"}), ("assets", [{}])]:
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                r.validate_draft(self.draft() | {field: bad}, empty=True)

    def test_pair_stays_original_not_recovery_commit(self):
        pair = {"schema_version": 1, "tag": r.TAG,
                "worldedit": {"repository": r.r.EDITOR, "sha": r.SOURCE, "version": "0.3.1"},
                "worldline": {"repository": r.r.CORE, "sha": r.CORE, "version": "0.3.0"}}
        r.validate_pair(pair)
        pair["worldedit"]["sha"] = "a" * 40
        with self.assertRaises(RuntimeError): r.validate_pair(pair)

    def test_missing_list_entry_retries_only_after_direct_id_validation(self):
        with patch.object(r.r, "pages", side_effect=[[], [self.draft()]]), \
             patch.object(r.r, "api", return_value=self.draft()) as direct, patch.object(r.time, "sleep") as wait:
            r.unique_draft()
            direct.assert_called_once()
            wait.assert_called_once_with(1)

    def test_duplicate_or_other_id_never_retries(self):
        for values in [[self.draft(), self.draft()], [self.draft() | {"id": 1}]]:
            with self.subTest(values=values), patch.object(r.r, "pages", return_value=values), \
                 patch.object(r.time, "sleep") as wait, self.assertRaises(RuntimeError):
                r.unique_draft()
            wait.assert_not_called()

    def test_list_missing_is_bounded(self):
        with patch.object(r.r, "pages", return_value=[]), patch.object(r.r, "api", return_value=self.draft()), \
             patch.object(r.time, "sleep") as wait, self.assertRaises(RuntimeError):
            r.unique_draft()
        self.assertEqual(wait.call_count, 3)

    def test_appearing_draft_with_wrong_identity_aborts(self):
        with patch.object(r.r, "pages", return_value=[]), \
             patch.object(r.r, "api", return_value=self.draft() | {"target_commitish": "a" * 40}), \
             patch.object(r.time, "sleep") as wait, self.assertRaises(RuntimeError):
            r.unique_draft()
        wait.assert_not_called()


if __name__ == "__main__":
    unittest.main()
