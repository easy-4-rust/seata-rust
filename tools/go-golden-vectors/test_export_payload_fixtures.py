import json
from pathlib import Path
import tempfile
import unittest

from export_payload_fixtures import export_fixtures

REPO = Path(__file__).resolve().parents[2]


class FixtureExportTest(unittest.TestCase):
    def setUp(self):
        root = REPO / "target"
        root.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="payload_export_", dir=root)
        self.base = Path(self.temp.name).resolve()
        self.assertTrue(self.base.is_relative_to(root.resolve()))
        self.addCleanup(self.temp.cleanup)
        self.source = self.base / "input.json"
        self.rows = json.loads((REPO / "tools/go-golden-vectors/expected.json").read_text())
        self.source.write_text(json.dumps(self.rows), encoding="utf-8")
        self.destination = self.base / "payloads"

    def test_exports_all_payloads_with_type_prefix_and_stable_manifest(self):
        try:
            manifest = export_fixtures(self.source, self.destination)
        except ValueError as error:
            self.fail(f"Valid frozen corpus must export: {error}")
        self.assertEqual(manifest["sample_count"], 29)
        self.assertEqual(len(list(self.destination.glob("*.bin"))), 29)
        self.assertEqual(
            (self.destination / "GlobalBeginRequest.bin").read_bytes(),
            bytes.fromhex("0001000007d000125365617461476f5472616e73616374696f6e"),
        )
        before = {p.name: p.read_bytes() for p in self.destination.iterdir()}
        self.assertEqual(export_fixtures(self.source, self.destination), manifest)
        self.assertEqual({p.name: p.read_bytes() for p in self.destination.iterdir()}, before)

    def test_bad_type_prefix_prevents_any_output(self):
        self.rows[-1]["serialized_hex"] = "ffff" + self.rows[-1]["body_hex"]
        self.source.write_text(json.dumps(self.rows))
        with self.assertRaises(ValueError):
            export_fixtures(self.source, self.destination)
        self.assertFalse(self.destination.exists())

    def test_differing_existing_file_prevents_partial_update(self):
        self.destination.mkdir()
        saved = self.destination / "GlobalBeginResponse.bin"
        saved.write_bytes(b"preserve this user artifact")
        with self.assertRaises(ValueError):
            export_fixtures(self.source, self.destination)
        self.assertEqual(list(self.destination.iterdir()), [saved])
        self.assertEqual(saved.read_bytes(), b"preserve this user artifact")

    def test_duplicate_or_path_traversal_sample_names_are_rejected(self):
        for name in ("../outside", self.rows[0]["name"]):
            self.rows[1]["name"] = name
            self.source.write_text(json.dumps(self.rows))
            with self.assertRaises(ValueError):
                export_fixtures(self.source, self.destination)
            self.assertFalse(self.destination.exists())

    def test_crlf_and_lf_checkout_export_the_same_manifest(self):
        text = json.dumps(self.rows, indent=2) + "\n"
        self.source.write_bytes(text.replace("\n", "\r\n").encode("utf-8"))
        before = export_fixtures(self.source, self.destination)
        self.source.write_bytes(text.encode("utf-8"))
        try:
            after = export_fixtures(self.source, self.destination)
        except ValueError as error:
            self.fail(f"Line-ending-only changes must preserve the exported corpus: {error}")
        self.assertEqual(before, after)

    def test_case_insensitive_filename_collisions_prevent_any_output(self):
        self.rows[1]["name"] = self.rows[0]["name"].lower()
        self.source.write_text(json.dumps(self.rows))
        with self.assertRaises(ValueError):
            export_fixtures(self.source, self.destination)
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
