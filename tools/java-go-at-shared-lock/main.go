// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	_ "github.com/go-sql-driver/mysql"
	"os"
	"path/filepath"
	"seata.apache.org/seata-go/v2/pkg/client"
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
func main() {
	check(len(os.Args) == 2, "One owned directory required")
	work, err := filepath.Abs(os.Args[1])
	must(err)
	check(filepath.VolumeName(work) == "D:" || filepath.VolumeName(work) == "E:", "D/E only")
	var cfg struct {
		DSN string `json:"dsn"`
	}
	raw, err := os.ReadFile(filepath.Join(work, "database.json"))
	must(err)
	must(json.Unmarshal(raw, &cfg))
	client.InitPath(filepath.Join(work, "seatago.yml"))
	direct, err := sql.Open("mysql", cfg.DSN)
	must(err)
	defer direct.Close()
	must(direct.Ping())
	proxy, err := sql.Open("seata-at-mysql", cfg.DSN)
	must(err)
	defer proxy.Close()
	must(proxy.Ping())
	err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "shared-lock-go-held", Timeout: 180 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
		xid := tm.GetXID(ctx)
		check(xid != "", "Original Go Begin missing")
		begin, e := json.Marshal(map[string]interface{}{"xid": xid, "process_id": os.Getpid()})
		must(e)
		must(os.WriteFile(filepath.Join(work, "begun.json"), begin, 0600))
		await(work, "start-update.ready")
		result, e := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance - 100 WHERE id = 1")
		if e != nil {
			return e
		}
		count, e := result.RowsAffected()
		if e != nil {
			return e
		}
		check(count == 1, "Actual Go row differs")
		var branch int64
		var undo []byte
		must(direct.QueryRow("SELECT branch_id,rollback_info FROM undo_log WHERE xid=?", xid).Scan(&branch, &undo))
		check(branch > 0 && len(undo) > 0, "Original Go Undo absent")
		phase, e := json.Marshal(map[string]interface{}{"xid": xid, "branch_id": branch, "undo_hex": hex.EncodeToString(undo), "process_id": os.Getpid()})
		must(e)
		must(os.WriteFile(filepath.Join(work, "phase-one.json"), phase, 0600))
		fmt.Println("REAL_GO_PHASE_ONE", xid, branch, os.Getpid())
		await(work, "release.ready")
		return nil
	})
	must(err)
	must(os.WriteFile(filepath.Join(work, "finished.ready"), []byte("Original TM commit returned\n"), 0600))
	fmt.Println("REAL_GO_GLOBAL_COMMIT", os.Getpid())
	await(work, "stop.ready")
}
