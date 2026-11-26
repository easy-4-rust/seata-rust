// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/tm"
	"strconv"
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
	tokenBytes := sha256.Sum256([]byte(fmt.Sprintf("%s/%d/10", bac.Xid, bac.BranchId)))
	preparedToken := hex.EncodeToString(tokenBytes[:])
	tx, err := a.db.BeginTx(ctx, nil)
	if err != nil {
		return false, err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, "INSERT INTO commit_reservation(xid,branch_id,phase,effect_count,prepared_units,prepared_token) VALUES (?,?,'prepared',0,10,?)", bac.Xid, bac.BranchId, preparedToken); err != nil {
		return false, err
	}
	if _, err = tx.ExecContext(ctx, "UPDATE commit_account SET reserved=reserved+10 WHERE id=1"); err != nil {
		return false, err
	}
	if err = tx.Commit(); err != nil {
		return false, err
	}
	bac.ActionContext["prepared-units"] = "10"
	bac.ActionContext["prepared-branch"] = strconv.FormatInt(bac.BranchId, 10)
	bac.ActionContext["prepared-token"] = preparedToken
	fmt.Println("REAL_TCC_CONTEXT_PREPARED", bac.Xid, bac.BranchId, preparedToken, os.Getpid())
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
	// 回调必须携带Prepare后真实生成并报告的值；缺失时拒绝，不从数据库补造。
	unitsText, unitsOK := bac.ActionContext["prepared-units"].(string)
	branchText, branchOK := bac.ActionContext["prepared-branch"].(string)
	token, tokenOK := bac.ActionContext["prepared-token"].(string)
	units, parseError := strconv.Atoi(unitsText)
	if !unitsOK || !branchOK || !tokenOK || parseError != nil || units != 10 || branchText != strconv.FormatInt(bac.BranchId, 10) || len(token) != 64 {
		fmt.Println("REAL_TCC_CONTEXT_REJECTED", bac.Xid, bac.BranchId, os.Getpid())
		return false, errors.New("TCC callback lacks original prepared context")
	}
	tx, err := a.db.BeginTx(ctx, nil)
	if err != nil {
		return false, err
	}
	defer tx.Rollback()
	var phase string
	var effects int
	var storedUnits int
	var storedToken string
	if err = tx.QueryRowContext(ctx, "SELECT phase,effect_count,prepared_units,prepared_token FROM commit_reservation WHERE xid=? AND branch_id=? FOR UPDATE", bac.Xid, bac.BranchId).Scan(&phase, &effects, &storedUnits, &storedToken); err != nil {
		return false, err
	}
	if storedUnits != units || storedToken != token {
		return false, errors.New("TCC prepared context differs from durable business row")
	}
	fmt.Println("REAL_TCC_CONTEXT_ACCEPTED", bac.Xid, bac.BranchId, token, os.Getpid())
	expected := "cancelled"
	if commit {
		expected = "committed"
	}
	if phase == "prepared" {
		query := "UPDATE commit_account SET reserved=reserved-? WHERE id=1 AND reserved>=?"
		if commit {
			query = "UPDATE commit_account SET reserved=reserved-?,confirmed=confirmed+? WHERE id=1 AND reserved>=?"
		}
		arguments := []interface{}{units, units}
		if commit {
			arguments = []interface{}{units, units, units}
		}
		result, e := tx.ExecContext(ctx, query, arguments...)
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
