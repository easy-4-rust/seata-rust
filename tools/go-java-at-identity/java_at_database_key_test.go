// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"testing"
)

func TestJavaATImageCannotChangeDatabaseIdentity(t *testing.T) {
	for _, hasMeta := range []bool{true, false} {
		for _, schema := range []bool{false, true} {
			name := "missing_metadata"
			if hasMeta {
				name = "cached_metadata"
			}
			if schema {
				name += "_schema"
			}
			t.Run(name, func(t *testing.T) {
				im := javaIdentityImage("accounts", int64(1))
				im.TableMetaKey = &types.TableMetaKey{DBName: "other", TableName: "accounts"}
				if schema {
					im.TableMetaKey.DBName = "orders"
					im.TableMetaKey.Schema = "other"
				}
				if !hasMeta {
					im.TableMeta = nil
				}
				if _, err := javaATLockKeys(javaIdentityContext(im), &DBResource{dbName: "orders"}); err == nil {
					t.Fatal("cross-database image bypassed explicit resource identity")
				}
			})
		}
	}
}
