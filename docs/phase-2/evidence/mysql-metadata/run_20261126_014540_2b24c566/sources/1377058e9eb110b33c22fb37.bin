-- 006 原回执的成功报告持久发送门闩；升级前停写并排空旧实验写入器。
-- 旧 Armed 行可能已报告；默认 1 保守禁止跨进程重发。新行由写入器显式置 0。
ALTER TABLE seata_rs_at_commit_attempt
    ADD COLUMN report_started TINYINT UNSIGNED NOT NULL DEFAULT 1 AFTER attempt_state;
