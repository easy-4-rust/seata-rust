-- 停止旧写入器并排空在途事务后显式应用；历史行不补造摘要。
ALTER TABLE seata_rs_at_branch_v1 ADD COLUMN undo_sha256 BINARY(32) NULL DEFAULT NULL AFTER first_phase_committed;
