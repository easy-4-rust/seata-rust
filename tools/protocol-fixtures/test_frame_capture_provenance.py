"""实际Java类来源分类门禁；防止普通生产类被当作动态生成类绕过校验。"""
import unittest
from capture_formal_frames import generated_owner


class ClassProvenanceTest(unittest.TestCase):
    def test_actual_lambda_shape_requires_its_exact_owner_source(self):
        owner = 'org.apache.seata.common.thread.NamedThreadFactory'
        name = owner + '$$Lambda$59/0x000001b28b0fe6d8'
        self.assertEqual(generated_owner(name, owner), owner)
        self.assertIsNone(generated_owner(name, 'shared objects file'))
        self.assertIsNone(generated_owner(name, 'org.apache.seata.other.Class'))

    def test_actual_cglib_requires_lookup_source(self):
        owner = 'org.apache.seata.config.FileConfiguration'
        name = owner + '$$EnhancerByCGLIB$$9e6bf6fd'
        self.assertEqual(generated_owner(name, '__JVM_LookupDefineClass__'), owner)
        self.assertIsNone(generated_owner(name, owner))

    def test_ordinary_nonfile_production_class_is_never_generated(self):
        for source in ['shared objects file', '__JVM_LookupDefineClass__', 'unknown']:
            self.assertIsNone(generated_owner('org.apache.seata.serializer.seata.protocol.GlobalBeginRequestCodec', source))

    def test_generated_substrings_without_real_shape_are_rejected(self):
        for name in ['org.apache.seata.Codec$$Lambda', 'org.apache.seata.Codec$$Lambda$1/not-an-address',
                     'org.apache.seata.Codec$$EnhancerByCGLIB$$bad-name!']:
            self.assertIsNone(generated_owner(name, '__JVM_LookupDefineClass__'))


if __name__ == '__main__':
    unittest.main()
