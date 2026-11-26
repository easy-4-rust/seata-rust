// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"context"
	"errors"
	"fmt"
	"sort"
	"strings"
	"unicode/utf8"

	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/util"
)

// javaATLockKeys从原镜像和真实主键次序构造Java命名空间，不改变Undo原字节。
func javaATLockKeys(ctx *types.TransactionContext, res *DBResource) (string, error) {
	invalid := errors.New("Java AT: reliable primary-key metadata and unambiguous lock encoding required")
	if ctx == nil || ctx.RoundImages == nil {
		return "", invalid
	}
	images := append(append(types.RecordImages{}, ctx.RoundImages.BeofreImages()...), ctx.RoundImages.AfterImages()...)
	keys := map[string]struct{}{}
	for _, image := range images {
		if image == nil {
			return "", invalid
		}
		if len(image.Rows) == 0 {
			continue
		}
		if res != nil && image.TableMetaKey != nil && (image.TableMetaKey.DBName != res.dbName || image.TableMetaKey.Schema != "") {
			return "", invalid
		}
		meta := image.TableMeta
		if meta == nil && res != nil && res.metaCache != nil {
			key := types.TableMetaKey{DBName: res.dbName, TableName: image.TableName}
			if image.TableMetaKey != nil {
				key = *image.TableMetaKey
			}
			if key.DBName != res.dbName || key.Schema != "" {
				return "", invalid
			}
			var err error
			meta, err = res.metaCache.GetTableMeta(context.Background(), key)
			if err != nil {
				return "", invalid
			}
		}
		if meta == nil || meta.TableName == "" || meta.TableName != image.TableName || strings.ContainsAny(meta.TableName, ":,;\x00") {
			return "", invalid
		}
		names, orderErr := javaATPrimaryKeyOrder(meta)
		if orderErr != nil || len(names) == 0 {
			return "", invalid
		}
		rows := make([]string, 0, len(image.Rows))
		for _, row := range image.Rows {
			columns := make(map[string]types.ColumnImage, len(row.Columns))
			for _, column := range row.Columns {
				name := util.DelEscape(column.ColumnName, types.DBTypeMySQL)
				if _, exists := columns[name]; exists {
					return "", invalid
				}
				columns[name] = column
			}
			values := make([]string, 0, len(names))
			for _, name := range names {
				column, found := columns[util.DelEscape(name, types.DBTypeMySQL)]
				if !found || column.Value == nil || column.KeyType != types.IndexTypePrimaryKey {
					return "", invalid
				}
				columnMeta, known := meta.Columns[util.DelEscape(name, types.DBTypeMySQL)]
				if !known {
					return "", invalid
				}
				value, valid := javaATPrimaryValue(column.Value, columnMeta)
				if !valid {
					return "", invalid
				}
				if strings.ContainsAny(value, ",;\x00") {
					return "", invalid
				}
				values = append(values, value)
			}
			rows = append(rows, strings.Join(values, "_"))
		}
		keys[meta.TableName+":"+strings.Join(rows, ",")] = struct{}{}
	}
	result := make([]string, 0, len(keys))
	for key := range keys {
		result = append(result, key)
	}
	sort.Strings(result)
	return strings.Join(result, ";"), nil
}

// JDBC类型决定值语义；未有固定Connector/J差分证据的类型不得猜测。
func javaATPrimaryValue(input interface{}, meta types.ColumnMeta) (string, bool) {
	jdbc := meta.DatabaseType
	if jdbc == 0 {
		switch strings.ToUpper(meta.DatabaseTypeString) {
		case "INT", "INTEGER", "MEDIUMINT":
			jdbc = 4
		case "BIGINT":
			jdbc = -5
		case "SMALLINT":
			jdbc = 5
		case "VARCHAR":
			jdbc = 12
		}
	}
	switch jdbc {
	case -5, 4, 5:
		switch input.(type) {
		case int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64:
			return fmt.Sprint(input), true
		}
	case 12:
		switch value := input.(type) {
		case string:
			return value, utf8.ValidString(value)
		case []byte:
			return string(value), utf8.Valid(value)
		}
	}
	return "", false
}
