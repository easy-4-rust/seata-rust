-- 显式扩展迁移。停止旧写入器后执行，保留已有行版本及原 Undo 表。
-- 默认零只表示没有本地提交见证，不证明任何旧业务尝试已回滚。
ALTER TABLE seata_rs_at_branch_v1
    ADD COLUMN first_phase_committed TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER commit_terminal;
