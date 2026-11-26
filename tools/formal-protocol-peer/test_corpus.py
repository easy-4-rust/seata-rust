"""跨语言比较器必须拒绝字段、字节、集合和类型的真实偏差。"""
import copy
import unittest
from corpus import compare


class ComparisonTest(unittest.TestCase):
    def setUp(self):
        self.cases=[{'id':'x','kind':'payload','hex':'0001','accepted':True,'semantic':{'type':1,'fields':[True,None,72623859790382856]}}]
        self.rows={'x':{'status':'accepted','semantic':copy.deepcopy(self.cases[0]['semantic']),'hex':'0001'}}

    def test_exact_reference_passes(self):
        self.assertEqual(compare(self.cases,self.rows,self.rows)['accepted'],1)

    def test_same_wire_with_wrong_semantics_fails(self):
        bad=copy.deepcopy(self.rows);bad['x']['semantic']['fields'][1]=''
        with self.assertRaises(ValueError):compare(self.cases,bad,bad)

    def test_bool_is_not_numeric_one_and_large_integer_is_exact(self):
        for replacement in [1,0]:
            bad=copy.deepcopy(self.rows);bad['x']['semantic']['fields'][0]=replacement
            with self.assertRaises(ValueError):compare(self.cases,bad,self.rows)
        bad=copy.deepcopy(self.rows);bad['x']['semantic']['fields'][2]+=1
        with self.assertRaises(ValueError):compare(self.cases,bad,self.rows)

    def test_missing_or_extra_identity_fails(self):
        for bad in [{},dict(self.rows,y=self.rows['x'])]:
            with self.assertRaises(ValueError):compare(self.cases,bad,self.rows)

    def test_wrong_encoding_fails(self):
        bad=copy.deepcopy(self.rows);bad['x']['hex']='0002'
        with self.assertRaises(ValueError):compare(self.cases,self.rows,bad)

    def test_panic_is_not_rejection(self):
        cases=[{'id':'bad','kind':'payload','hex':'','accepted':False,'semantic':None}]
        rejected={'bad':{'status':'rejected','semantic':None,'hex':''}}
        panic={'bad':{'status':'panic','semantic':None,'hex':''}}
        self.assertEqual(compare(cases,rejected,rejected)['rejected'],1)
        with self.assertRaises(ValueError):compare(cases,panic,rejected)

    def test_encoder_failure_after_decode_is_not_rejection(self):
        cases=[{'id':'bad','kind':'payload','hex':'','accepted':False,'semantic':None}]
        rejected={'bad':{'status':'rejected','semantic':None,'hex':''}}
        encoder_error={'bad':{'status':'encode_error','semantic':{'type':1,'fields':[]},'hex':''}}
        with self.assertRaises(ValueError):compare(cases,encoder_error,rejected)


if __name__=='__main__':unittest.main()
