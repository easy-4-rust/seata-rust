import unittest
from pathlib import Path
from rm_metadata_source import safe_member,check_member_authority
class PortablePathsTest(unittest.TestCase):
    def test_normal_classpath_member_stays_under_selected_root(self):
        root=Path("D:/Temp/seata-rm-metadata-path-test")
        self.assertEqual(safe_member(root,"cp/4/org/example/Value.class","cp"),(root/"cp/4/org/example/Value.class").resolve())
    def test_traversal_drives_absolute_paths_empty_parts_and_wrong_prefix_are_rejected(self):
        for name in ("cp/../escape","cp/4/../../escape","F:/escape","/cp/4/x","cp//x","cp/./x","portable-inputs/x","cp\\4\\x","cp/E:/x"):
            with self.subTest(name=name),self.assertRaises(AssertionError):
                safe_member(Path("D:/Temp/seata-rm-metadata-path-test"),name,"cp")
    def test_rehashed_or_unknown_member_cannot_replace_sealed_authority(self):
        expected={"cp/4/original.class":"fixed-original-digest"}
        check_member_authority({"path":"cp/4/original.class","sha256":"fixed-original-digest"},expected)
        for row in ({"path":"cp/4/original.class","sha256":"attacker-rehashed-content"},{"path":"cp/4/other.class","sha256":"fixed-original-digest"}):
            with self.subTest(row=row),self.assertRaises(AssertionError):
                check_member_authority(row,expected)
if __name__=="__main__":unittest.main()
