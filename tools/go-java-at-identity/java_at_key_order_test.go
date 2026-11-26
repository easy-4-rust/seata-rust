// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"encoding/json"
	"os"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"testing"
)

func TestJavaATActualJavaPrimaryKeyOrder(t *testing.T) {
	path := os.Getenv("SEATA_JAVA_AT_KEY_VECTORS")
	if path == "" {
		t.Fatal("actual pinned Java key fixture required")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Input []string `json:"input"`
		Order []string `json:"order"`
		Key   string   `json:"key"`
	}
	if err = json.Unmarshal(raw, &cases); err != nil {
		t.Fatal(err)
	}
	if len(cases) != 214 {
		t.Fatal("all original JVM cases required")
	}
	for _, c := range cases {
		t.Run(c.Key, func(t *testing.T) {
			im := javaIdentityImage("AccountLedger")
			columns := []types.ColumnMeta{}
			row := types.RowImage{}
			for i, name := range c.Input {
				column := types.ColumnMeta{ColumnName: name, DatabaseType: -5, DatabaseTypeString: "BIGINT"}
				columns = append(columns, column)
				row.Columns = append(row.Columns, types.ColumnImage{ColumnName: name, KeyType: types.IndexTypePrimaryKey, ColumnType: -5, Value: int64(i + 1)})
				im.TableMeta.Columns[name] = column
			}
			im.TableMeta.Indexs["PRIMARY"] = types.IndexMeta{IType: types.IndexTypePrimaryKey, Columns: columns}
			im.Rows = []types.RowImage{row}
			got, err := javaATLockKeys(javaIdentityContext(im), nil)
			if err != nil || got != c.Key {
				t.Fatalf("original Java key=%q actual=%q err=%v", c.Key, got, err)
			}
			if c.Input[0] == "z" && reflect.DeepEqual(c.Input, c.Order) {
				t.Fatal("Java physical-order counterexample absent")
			}
		})
	}
}
