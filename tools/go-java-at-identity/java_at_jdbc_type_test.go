// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"testing"
)

func TestJavaATJDBCPrimaryKeyPolicy(t *testing.T) {
	cases := []struct {
		name   string
		jdbc   int32
		value  interface{}
		want   string
		reject bool
	}{
		{"bigint", -5, int64(1), "accounts:1", false},
		{"unsigned_bigint", -5, uint64(18446744073709551615), "accounts:18446744073709551615", false},
		{"jdbc_varchar_bytes", 12, []byte("订单甲"), "accounts:订单甲", false},
		{"tinyint_boolean_not_yet_certified", -6, int64(1), "", true},
		{"bit_not_yet_certified", -7, []byte{1}, "", true},
		{"double_not_yet_certified", 8, float64(1), "", true},
		{"unknown_metadata", 0, int64(1), "", true},
		{"bigint_string_mismatch", -5, "1", "", true},
		{"varchar_number_mismatch", 12, int64(1), "", true},
		{"varchar_invalid_utf8", 12, []byte{255}, "", true},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			im := javaIdentityImage("accounts", c.value)
			im.TableMeta.Columns["id"] = types.ColumnMeta{ColumnName: "id", DatabaseType: c.jdbc}
			got, err := javaATLockKeys(javaIdentityContext(im), nil)
			if c.reject {
				if err == nil {
					t.Fatal("unverified JDBC encoding accepted")
				}
				return
			}
			if err != nil || got != c.want {
				t.Fatalf("key=%q want=%q err=%v", got, c.want, err)
			}
		})
	}
}
