// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	_ "github.com/go-sql-driver/mysql"
	"os"
	"path/filepath"
	"seata.apache.org/seata-go/v2/pkg/client"
	atasql "seata.apache.org/seata-go/v2/pkg/datasource/sql"
	"seata.apache.org/seata-go/v2/pkg/tm"
	"time"
)

func must(err error) {
	if err != nil {
		panic(err)
	}
}
func check(ok bool, label string) {
	if !ok {
		panic(label)
	}
}
func await(work, name string) {
	end := time.Now().Add(120 * time.Second)
	for {
		if _, err := os.Stat(filepath.Join(work, name)); err == nil {
			return
		}
		check(time.Now().Before(end), "Owned marker timeout "+name)
		time.Sleep(20 * time.Millisecond)
	}
}
func save(work, name string, value interface{}) {
	raw, err := json.Marshal(value)
	must(err)
	file, err := os.OpenFile(filepath.Join(work, name), os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	must(err)
	_, err = file.Write(raw)
	must(err)
	must(file.Close())
}
func main() {
	check(len(os.Args) == 2, "One owned directory required")
	work, err := filepath.Abs(os.Args[1])
	must(err)
	check(filepath.VolumeName(work) == "D:" || filepath.VolumeName(work) == "E:", "D/E only")
	var cfg struct {
		DSN     string `json:"dsn"`
		JDBCURL string `json:"jdbc_url"`
		End     string `json:"end_action"`
	}
	raw, err := os.ReadFile(filepath.Join(work, "database.json"))
	must(err)
	must(json.Unmarshal(raw, &cfg))
	check(cfg.End == "commit" || cfg.End == "rollback", "Explicit end action required")
	client.InitPath(filepath.Join(work, "seatago.yml"))
	direct, err := sql.Open("mysql", cfg.DSN)
	must(err)
	defer direct.Close()
	must(direct.Ping())
	proxy, err := atasql.OpenJavaATMySQL(cfg.DSN, cfg.JDBCURL)
	must(err)
	defer proxy.Close()
	must(proxy.Ping())
	duplicate, duplicateErr := atasql.OpenJavaATMySQL(cfg.DSN, cfg.JDBCURL)
	check(duplicate == nil && duplicateErr != nil, "same-ID Open silently replaced registered resource")
	save(work, "duplicate-open.json", map[string]interface{}{"rejected": true, "process_id": os.Getpid()})
	var blockedXID string
	err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "shared-lock-go-rejected", Timeout: 180 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
		blockedXID = tm.GetXID(ctx)
		save(work, "begun.json", map[string]interface{}{"xid": blockedXID, "process_id": os.Getpid()})
		await(work, "start-update.ready")
		_, attempt := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance - 100 WHERE id = 1")
		return attempt
	})
	check(err != nil, "TC shared lock did not reject conflicting AT update")
	var balance int64
	var count int
	must(direct.QueryRow("SELECT balance FROM accounts WHERE id=1").Scan(&balance))
	must(direct.QueryRow("SELECT COUNT(*) FROM undo_log WHERE xid=?", blockedXID).Scan(&count))
	check(balance == 900 && count == 0, "rejected AT update left local business effect or Undo")
	save(work, "blocked.json", map[string]interface{}{"xid": blockedXID, "process_id": os.Getpid(), "balance": balance, "undo_rows": count, "error": err.Error()})
	fmt.Println("REAL_GO_LOCK_REJECTED", blockedXID, os.Getpid())
	await(work, "start-success.ready")
	forced := errors.New("owned explicit new transaction rollback")
	err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "shared-lock-go-new-" + cfg.End, Timeout: 180 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
		xid := tm.GetXID(ctx)
		check(xid != "" && xid != blockedXID, "second transaction must have a new original XID")
		result, e := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance - 100 WHERE id = 1")
		if e != nil {
			return e
		}
		changed, e := result.RowsAffected()
		if e != nil {
			return e
		}
		check(changed == 1, "new phase-one row differs")
		var branch int64
		var undo []byte
		must(direct.QueryRow("SELECT branch_id,rollback_info FROM undo_log WHERE xid=?", xid).Scan(&branch, &undo))
		check(branch > 0 && len(undo) > 0, "new successful phase-one Undo absent")
		must(direct.QueryRow("SELECT balance FROM accounts WHERE id=1").Scan(&balance))
		save(work, "phase-one.json", map[string]interface{}{"xid": xid, "branch_id": branch, "undo_hex": hex.EncodeToString(undo), "balance": balance, "process_id": os.Getpid()})
		fmt.Println("REAL_GO_NEW_PHASE_ONE", xid, branch, os.Getpid())
		await(work, "release.ready")
		if cfg.End == "rollback" {
			return forced
		}
		return nil
	})
	if cfg.End == "rollback" {
		check(errors.Is(err, forced), "explicit rollback result differs")
	} else {
		must(err)
	}
	save(work, "finished.ready", map[string]interface{}{"end_action": cfg.End})
	fmt.Println("REAL_GO_NEW_GLOBAL_END", cfg.End, os.Getpid())
	await(work, "stop.ready")
}
