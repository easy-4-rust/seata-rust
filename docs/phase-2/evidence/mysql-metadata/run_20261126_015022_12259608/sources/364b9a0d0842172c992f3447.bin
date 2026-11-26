-- 显式扩展迁移：先停止旧写入器，保留原行版本、方向、回滚终态及全部身份。
-- 旧 phase=1 不推断为正式 TC 决定；须再次收到真实提交回调才可以授权清理。
ALTER TABLE seata_rs_at_branch_v1
    ADD COLUMN commit_authorized TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER terminal,
    ADD COLUMN commit_terminal TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER commit_authorized;
