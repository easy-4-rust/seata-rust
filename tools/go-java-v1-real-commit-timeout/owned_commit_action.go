// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/tm"
)

// OwnedCommitAction 是本次真实TCC业务：预留10个单位，确认时只产生一次持久效果。
// 原Go TCCResourceManager负责注册、原始回调与协议响应；业务通过原XID/BranchId行锁保证幂等。
type OwnedCommitAction struct{ db *sql.DB }

func (a *OwnedCommitAction) GetActionName() string { return "owned-real-commit-action" }
func (a *OwnedCommitAction) Prepare(ctx context.Context, _ interface{}) (bool, error) {
	bac := tm.GetBusinessActionContext(ctx)
	if bac == nil || bac.Xid == "" || bac.BranchId <= 0 {
		return false, errors.New("TCC prepare lacks actual registered branch")
	}
	tx, err := a.db.BeginTx(ctx, nil)
	if err != nil {
		return false, err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, "INSERT INTO commit_reservation(xid,branch_id,phase,effect_count) VALUES (?,?,'prepared',0)", bac.Xid, bac.BranchId); err != nil {
		return false, err
	}
	if _, err = tx.ExecContext(ctx, "UPDATE commit_account SET reserved=reserved+10 WHERE id=1"); err != nil {
		return false, err
	}
	if err = tx.Commit(); err != nil {
		return false, err
	}
	fmt.Println("REAL_TCC_PREPARED", bac.Xid, bac.BranchId, os.Getpid())
	return true, nil
}
func (a *OwnedCommitAction) Commit(ctx context.Context, bac *tm.BusinessActionContext) (bool, error) {
	return a.finish(ctx, bac, true)
}
func (a *OwnedCommitAction) Rollback(ctx context.Context, bac *tm.BusinessActionContext) (bool, error) {
	return a.finish(ctx, bac, false)
}
func (a *OwnedCommitAction) finish(ctx context.Context, bac *tm.BusinessActionContext, commit bool) (bool, error) {
	if bac == nil || bac.Xid == "" || bac.BranchId <= 0 || bac.ActionName != a.GetActionName() {
		return false, errors.New("TCC callback identity missing")
	}
	tx, err := a.db.BeginTx(ctx, nil)
	if err != nil {
		return false, err
	}
	defer tx.Rollback()
	var phase string
	var effects int
	if err = tx.QueryRowContext(ctx, "SELECT phase,effect_count FROM commit_reservation WHERE xid=? AND branch_id=? FOR UPDATE", bac.Xid, bac.BranchId).Scan(&phase, &effects); err != nil {
		return false, err
	}
	expected := "cancelled"
	if commit {
		expected = "committed"
	}
	if phase == "prepared" {
		query := "UPDATE commit_account SET reserved=reserved-10 WHERE id=1 AND reserved>=10"
		if commit {
			query = "UPDATE commit_account SET reserved=reserved-10,confirmed=confirmed+10 WHERE id=1 AND reserved>=10"
		}
		result, e := tx.ExecContext(ctx, query)
		if e != nil {
			return false, e
		}
		count, e := result.RowsAffected()
		if e != nil {
			return false, e
		}
		if count != 1 {
			return false, errors.New("TCC reserved business units missing")
		}
		if commit {
			effects++
		}
		if _, err = tx.ExecContext(ctx, "UPDATE commit_reservation SET phase=?,effect_count=? WHERE xid=? AND branch_id=?", expected, effects, bac.Xid, bac.BranchId); err != nil {
			return false, err
		}
	} else if phase != expected {
		return false, errors.New("TCC opposite final phase cannot be changed")
	}
	if err = tx.Commit(); err != nil {
		return false, err
	}
	fmt.Println("REAL_TCC_FINISHED", bac.Xid, bac.BranchId, expected, effects, os.Getpid())
	return true, nil
}
