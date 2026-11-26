// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"testing"
)

func javaIdentityImage(table string, values ...interface{}) *types.RecordImage {
	meta := &types.TableMeta{TableName: table, Columns: map[string]types.ColumnMeta{"id": {ColumnName: "id", DatabaseType: -5, DatabaseTypeString: "BIGINT"}}, Indexs: map[string]types.IndexMeta{"PRIMARY": {IType: types.IndexTypePrimaryKey, Columns: []types.ColumnMeta{{ColumnName: "id"}}}}}
	image := &types.RecordImage{TableName: table, TableMeta: meta}
	for _, value := range values {
		if _, ok := value.(string); ok {
			meta.Columns["id"] = types.ColumnMeta{ColumnName: "id", DatabaseType: 12, DatabaseTypeString: "VARCHAR"}
		}
		image.Rows = append(image.Rows, types.RowImage{Columns: []types.ColumnImage{{ColumnName: "id", KeyType: types.IndexTypePrimaryKey, Value: value}}})
	}
	return image
}
func javaIdentityContext(images ...*types.RecordImage) *types.TransactionContext {
	ctx := types.NewTxCtx()
	ctx.RoundImages.AppendBeofreImages(images)
	for _, im := range images {
		if im != nil {
			ctx.LockKeys[im.TableName+":legacy"] = struct{}{}
		}
	}
	return ctx
}
func TestJavaATLockKeys(t *testing.T) {
	cases := []struct {
		name   string
		images []*types.RecordImage
		want   string
	}{
		{"case", []*types.RecordImage{javaIdentityImage("accounts", int64(1))}, "accounts:1"},
		{"mixed_case", []*types.RecordImage{javaIdentityImage("AccountLedger", int64(1))}, "AccountLedger:1"},
		{"rows", []*types.RecordImage{javaIdentityImage("accounts", int64(1), int64(2))}, "accounts:1,2"},
		{"dedupe", []*types.RecordImage{javaIdentityImage("accounts", int64(1)), javaIdentityImage("accounts", int64(1))}, "accounts:1"},
		{"multitable", []*types.RecordImage{javaIdentityImage("zeta", int64(2)), javaIdentityImage("accounts", int64(1))}, "accounts:1;zeta:2"},
		{"unicode", []*types.RecordImage{javaIdentityImage("orders", "订单甲")}, "orders:订单甲"},
		{"empty_rows", []*types.RecordImage{javaIdentityImage("accounts")}, ""},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got, err := javaATLockKeys(javaIdentityContext(c.images...), nil)
			if err != nil || got != c.want {
				t.Fatalf("actual key=%q want=%q err=%v", got, c.want, err)
			}
		})
	}
}
func TestJavaATCompositeKeyOrder(t *testing.T) {
	im := javaIdentityImage("orders", int64(7))
	im.TableMeta.Indexs["PRIMARY"] = types.IndexMeta{IType: types.IndexTypePrimaryKey, Columns: []types.ColumnMeta{{ColumnName: "id"}, {ColumnName: "tenant"}}}
	im.TableMeta.Columns["tenant"] = types.ColumnMeta{ColumnName: "tenant", DatabaseType: 12, DatabaseTypeString: "VARCHAR"}
	im.Rows[0].Columns = []types.ColumnImage{{ColumnName: "tenant", KeyType: types.IndexTypePrimaryKey, Value: "eu"}, {ColumnName: "id", KeyType: types.IndexTypePrimaryKey, Value: int64(7)}}
	got, err := javaATLockKeys(javaIdentityContext(im), nil)
	if err != nil || got != "orders:7_eu" {
		t.Fatalf("composite PK metadata order lost: %q %v", got, err)
	}
}
func TestJavaATLockKeysRejectIncompleteImage(t *testing.T) {
	cases := []struct {
		name  string
		alter func(*types.RecordImage)
	}{
		{"missing_meta", func(im *types.RecordImage) { im.TableMeta = nil }},
		{"missing_pk", func(im *types.RecordImage) { im.TableMeta.Indexs = map[string]types.IndexMeta{} }},
		{"missing_column", func(im *types.RecordImage) { im.Rows[0].Columns = nil }},
		{"nil_pk", func(im *types.RecordImage) { im.Rows[0].Columns[0].Value = nil }},
		{"comma_pk", func(im *types.RecordImage) { im.Rows[0].Columns[0].Value = "a,b" }},
		{"semicolon_pk", func(im *types.RecordImage) { im.Rows[0].Columns[0].Value = "a;b" }},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			im := javaIdentityImage("accounts", int64(1))
			c.alter(im)
			if _, err := javaATLockKeys(javaIdentityContext(im), nil); err == nil {
				t.Fatal("unsafe key accepted")
			}
		})
	}
}
