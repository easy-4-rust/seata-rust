-- 显式扩展迁移：先停止旧写入器；保留所有旧行及原 schema_version。
-- 新读取器将旧 version=1 + 默认 origin=1 视为原成功回执行。
ALTER TABLE seata_rs_at_branch_v1
    ADD COLUMN registration_origin TINYINT UNSIGNED NOT NULL DEFAULT 1 AFTER lock_key;
