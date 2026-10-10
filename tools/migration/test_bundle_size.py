import gzip
from pathlib import Path
import tempfile
import unittest
from bundle_size import asset_inventory, web_inventory, wasm_sections


class BundleEvidenceTests(unittest.TestCase):
    def test_sidecars_are_not_counted_as_runtime_downloads(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)
            (p/'game.wasm').write_bytes(b'game')
            (p/'game.wasm.gz').write_bytes(gzip.compress(b'game'))
            (p/'game.d.ts').write_text('declarations')
            (p/'raylib.data').write_bytes(b'asset')
            result=web_inventory(p)
        self.assertEqual(set(result['files']),{'game.wasm','raylib.data'})
        self.assertEqual(result['totals']['runtime']['bytes'],4)
        self.assertEqual(result['totals']['assets']['bytes'],5)
        self.assertGreater(result['existing_sidecar_bytes'],0)

    def test_same_size_changed_assets_do_not_pass_hash_comparison(self):
        with tempfile.TemporaryDirectory() as d:
            a,b=Path(d)/'a',Path(d)/'b';a.mkdir();b.mkdir()
            (a/'model').write_bytes(b'abc');(b/'model').write_bytes(b'xyz')
            self.assertNotEqual(asset_inventory(a),asset_inventory(b))

    def test_wasm_name_section_is_separate_from_code(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'test.wasm'
            p.write_bytes(b'\0asm\1\0\0\0'+b'\0\x05\x04name'+b'\x0a\x01\0')
            self.assertEqual(wasm_sections(p),[{'id':0,'name':'name','bytes':5},{'id':10,'name':'','bytes':1}])
            p.write_bytes(b'\0asm\1\0\0\0\x0a\x05\0')
            with self.assertRaises(ValueError):wasm_sections(p)


if __name__=='__main__':unittest.main()
