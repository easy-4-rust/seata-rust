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
	check(filepath.VolumeName(directory) == "D:" || filepath.VolumeName(directory) == "E:", "runtime outside D:/E:")
	data, err := os.ReadFile(filepath.Join(directory, "database.json"))
	must(err)
	var cfg struct {
		DSN             string `json:"dsn"`
		ReconnectReplay bool   `json:"reconnect_replay"`
		RecoveryMode    string `json:"recovery_mode"`
	}
	must(json.Unmarshal(data, &cfg))
	client.InitPath(filepath.Join(directory, "seatago.yml"))
	fmt.Println("FULL_CLIENT_INIT_RETURNED")
	direct, err := sql.Open("mysql", cfg.DSN)
	must(err)
	defer direct.Close()
	must(direct.Ping())
	// Open本身通过原RM路径向TC注册真实数据源，不手工构造Resource或伪造二阶段响应。
	proxy, err := sql.Open("seata-at-mysql", cfg.DSN)
	must(err)
	defer proxy.Close()
	must(proxy.Ping())
	fmt.Println("REAL_RESOURCE_OPENED")
	if cfg.RecoveryMode == "resume" {
		data, e := os.ReadFile(filepath.Join(directory, "recovery.json"))
		must(e)
		var prior struct {
			Xid       string `json:"xid"`
			BranchId  int64  `json:"branch_id"`
			UndoSha   string `json:"undo_sha256"`
			ProcessId int    `json:"process_id"`
		}
		must(json.Unmarshal(data, &prior))
		persisted := phaseOne(direct, prior.Xid, 900)
		check(persisted["branch_id"] == prior.BranchId && persisted["undo_sha256"] == prior.UndoSha, "business/Undo identity changed across DB and SDK restart")
		check(prior.ProcessId > 0 && prior.ProcessId != os.Getpid(), "recovery did not use a different SDK process")
		ctx := tm.InitSeataContext(context.Background())
		must(tm.GetGlobalTransactionManager().Rollback(ctx, &tm.GlobalTransaction{Xid: prior.Xid, TxRole: tm.Launcher}))
		awaitDatabase(direct, prior.Xid, 1000)
		states := tcFinished(prior.Xid)
		proof, e := json.Marshal(map[string]interface{}{"full_client_init": true, "recovery": persisted, "original_process": prior.ProcessId, "recovery_process": os.Getpid(), "restored_balance": 1000, "undo_rows": countUndo(direct, prior.Xid), "tc_states": states, "second_begin_issued": false, "scope": "normal Begin/Registered persistent replay after SDK process and MySQL restart; not TC retry-timeout16/17 recovery"})
		must(e)
		mark(directory, "sdk-finished.ready")
		fmt.Println("REAL_AT_RECOVERY_PASS " + string(proof))
		return
	}
	check(balance(direct) == 1000, "owned initial row differs")
	var rollbackXid string
	var rollbackPhase map[string]interface{}
	intentional := errors.New("owned forced business rollback")
	err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "real-at-rollback", Timeout: 60 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
		rollbackXid = tm.GetXID(ctx)
		check(rollbackXid != "", "full TM did not bind XID")
		result, e := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance - 100 WHERE id = 1")
		if e != nil {
			return e
		}
		changed, e := result.RowsAffected()
		if e != nil {
			return e
		}
		check(changed == 1, "rollback phase-one affected row differs")
		rollbackPhase = phaseOne(direct, rollbackXid, 900)
		row, _ := json.Marshal(rollbackPhase)
		fmt.Println("REAL_AT_PHASE_ONE " + string(row))
		if cfg.RecoveryMode == "prepare" {
			must(os.WriteFile(filepath.Join(directory, "recovery.json"), append(row, '\n'), 0600))
			mark(directory, "phase-one-exit.ready")
			fmt.Println("REAL_AT_PREPARED " + string(row))
			os.Exit(0)
		}
		if cfg.ReconnectReplay {
			mark(directory, "reconnect.request")
			await(directory, "resource-reregistered.ready")
		}
		return intentional
	})
	check(err != nil && strings.Contains(err.Error(), intentional.Error()), "business rollback did not propagate original failure")
	awaitDatabase(direct, rollbackXid, 1000)
	rollbackStates := tcFinished(rollbackXid)
	fmt.Println("REAL_AT_ROLLBACK_RESTORED")
	defenseStatus := -1
	if cfg.ReconnectReplay {
		mark(directory, "repeat-rollback.request")
		await(directory, "repeat-rollback-reply.ready")
		check(balance(direct) == 1000, "replayed rollback changed restored row")
		must(direct.QueryRow("SELECT log_status FROM undo_log WHERE xid=? AND branch_id=?", rollbackXid, rollbackPhase["branch_id"]).Scan(&defenseStatus))
		check(defenseStatus == 1, "missing persisted anti-suspension record")
	}
	var commitXid string
	var commitPhase map[string]interface{}
	err = tm.WithGlobalTx(context.Background(), &tm.GtxConfig{Name: "real-at-commit", Timeout: 60 * time.Second, Propagation: tm.Required}, func(ctx context.Context) error {
		commitXid = tm.GetXID(ctx)
		result, e := proxy.ExecContext(ctx, "UPDATE accounts SET balance = balance + 25 WHERE id = 1")
		if e != nil {
			return e
		}
		changed, e := result.RowsAffected()
		if e != nil {
			return e
		}
		check(changed == 1, "commit phase-one affected row differs")
		commitPhase = phaseOne(direct, commitXid, 1025)
		row, _ := json.Marshal(commitPhase)
		fmt.Println("REAL_AT_PHASE_ONE " + string(row))
		return nil
	})
	must(err)
	awaitDatabase(direct, commitXid, 1025)
	commitStates := tcFinished(commitXid)
	var totalUndo int
	must(direct.QueryRow("SELECT COUNT(*) FROM undo_log").Scan(&totalUndo))
	if cfg.ReconnectReplay {
		check(totalUndo == 1, "unexpected final undo count with defense record")
	} else {
		check(totalUndo == 0, "normal undo not cleaned")
	}
	proof, err := json.Marshal(map[string]interface{}{"full_client_init": true, "real_sql_proxy": true, "rollback": rollbackPhase, "commit": commitPhase, "rollback_restored_balance": 1000, "committed_balance": 1025, "final_undo_rows": totalUndo, "reconnect_replay": cfg.ReconnectReplay, "defense_status": defenseStatus, "tc_rollback_states": rollbackStates, "tc_commit_states": commitStates, "scope": "real normal MySQL AT and optional reconnect/duplicate paths; no Rust/timeout-recovery/restart certification"})
	must(err)
	must(os.WriteFile(filepath.Join(directory, "sdk-finished.ready"), []byte("owned normal AT assertions passed\n"), 0600))
	fmt.Println("REAL_AT_PASS " + string(proof))
}
