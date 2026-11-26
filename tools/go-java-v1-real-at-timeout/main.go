// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	_ "github.com/go-sql-driver/mysql"
	"os"
	"path/filepath"
	"seata.apache.org/seata-go/v2/pkg/client"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	"seata.apache.org/seata-go/v2/pkg/tm"
	"strings"
	"time"
)

func check(ok bool, label string) {
	if !ok {
		panic(label)
	}
}
func must(err error) {
	if err != nil {
		panic(err)
	}
}
func mark(directory, name string) {
	must(os.WriteFile(filepath.Join(directory, name), []byte("owned real AT marker\n"), 0600))
}
func await(directory, name string) {
	deadline := time.Now().Add(20 * time.Second)
	for {
		if _, err := os.Stat(filepath.Join(directory, name)); err == nil {
			return
		}
		check(time.Now().Before(deadline), "owned real AT marker timeout: "+name)
		time.Sleep(10 * time.Millisecond)
	}
}
func balance(db *sql.DB) int64 {
	var value int64
	must(db.QueryRow("SELECT balance FROM accounts WHERE id=1").Scan(&value))
	return value
}
func countUndo(db *sql.DB, xid string) int {
	var count int
	must(db.QueryRow("SELECT COUNT(*) FROM undo_log WHERE xid=?", xid).Scan(&count))
	return count
}
func phaseOne(db *sql.DB, xid string, want int64) map[string]interface{} {
	check(balance(db) == want, "phase-one business row not committed")
	var branch int64
	var data []byte
	var status int
	must(db.QueryRow("SELECT branch_id,rollback_info,log_status FROM undo_log WHERE xid=?", xid).Scan(&branch, &data, &status))
	check(branch > 0 && len(data) > 0 && status == 0, "phase-one Undo identity/blob/status missing")
	hash := sha256.Sum256(data)
	return map[string]interface{}{"xid": xid, "branch_id": branch, "balance": want, "undo_status": status, "undo_size": len(data), "undo_sha256": hex.EncodeToString(hash[:]), "undo_hex": hex.EncodeToString(data), "process_id": os.Getpid()}
}
func awaitDatabase(db *sql.DB, xid string, want int64) {
	deadline := time.Now().Add(15 * time.Second)
	for {
		if balance(db) == want && countUndo(db, xid) == 0 {
			return
		}
		check(time.Now().Before(deadline), "phase-two database/Undo result did not converge")
		time.Sleep(50 * time.Millisecond)
	}
}
func tcFinished(xid string) []int {
	deadline := time.Now().Add(15 * time.Second)
	var states []int
	for {
		value, e := getty.GetGettyRemotingClient().SendSyncRequest(message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}})
		must(e)
		reply := value.(message.GlobalStatusResponse)
		check(reply.ResultCode == 1, "TC status query rejected")
		state := int(reply.GlobalStatus)
		states = append(states, state)
		if state == 15 {
			return states
		}
		check((state == 8 || state == 9 || state == 10 || state == 11) && time.Now().Before(deadline), "unexpected or stuck TC ending state")
		time.Sleep(100 * time.Millisecond)
	}
}
func main() {
	check(len(os.Args) == 2, "one owned runtime directory required")
	directory, err := filepath.Abs(os.Args[1])
	must(err)
	check(filepath.VolumeName(directory) == "D:" || filepath.VolumeName(directory) == "E:", "runtime outside D/E")
	raw, err := os.ReadFile(filepath.Join(directory, "database.json"))
	must(err)
	var cfg struct {
		DSN  string `json:"dsn"`
		Mode string `json:"mode"`
	}
	must(json.Unmarshal(raw, &cfg))
	client.InitPath(filepath.Join(directory, "seatago.yml"))
	direct, err := sql.Open("mysql", cfg.DSN)
	must(err)
	defer direct.Close()
	must(direct.Ping())
	proxy, err := sql.Open("seata-at-mysql", cfg.DSN)
	must(err)
	defer proxy.Close()
	must(proxy.Ping())
	fmt.Println("REAL_TIMEOUT_PROCESS", cfg.Mode, os.Getpid())
	if cfg.Mode == "prepare" {
		intentional := errors.New("owned natural retry-timeout business rollback")
		err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "real-at-natural-timeout", Timeout: 120 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
			xid := tm.GetXID(ctx)
			check(xid != "", "actual TM did not bind XID")
			result, e := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance - 100 WHERE id = 1")
			if e != nil {
				return e
			}
			changed, e := result.RowsAffected()
			if e != nil {
				return e
			}
			check(changed == 1, "actual local phase one row differs")
			proof := phaseOne(direct, xid, 900)
			data, e := json.Marshal(proof)
			must(e)
			must(os.WriteFile(filepath.Join(directory, "recovery.json"), append(data, '\n'), 0600))
			fmt.Println("REAL_TIMEOUT_PHASE_ONE " + string(data))
			return intentional
		})
		check(err != nil && strings.Contains(err.Error(), intentional.Error()), "actual TM did not propagate the business failure")
	} else {
		check(cfg.Mode == "serve", "unknown owned mode")
	}
	mark(directory, "sdk-serving.ready")
	fmt.Println("REAL_TIMEOUT_SERVING", os.Getpid())
	deadline := time.Now().Add(180 * time.Second)
	for {
		if _, e := os.Stat(filepath.Join(directory, "finish.ready")); e == nil {
			break
		}
		check(time.Now().Before(deadline), "owned recovery controller timed out")
		time.Sleep(25 * time.Millisecond)
	}
	mark(directory, "sdk-finished.ready")
	fmt.Println("REAL_TIMEOUT_PROCESS_DONE", os.Getpid())
}
