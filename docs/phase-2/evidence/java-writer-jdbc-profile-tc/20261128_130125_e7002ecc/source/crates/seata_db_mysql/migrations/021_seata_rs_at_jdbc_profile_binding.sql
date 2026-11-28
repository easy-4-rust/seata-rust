-- 可选严格准入；部署方排空旧写入器与未完成分支后手动设置完整 JDBC 配置码。
-- 旧行保留 NULL，不从 TINYINT 布尔位推断 UTC 或精确 DATE/TIME 表示。
ALTER TABLE seata_rs_at_resource_binding
    ADD COLUMN jdbc_profile TINYINT UNSIGNED NULL,
    ADD CONSTRAINT ck_seata_rs_at_jdbc_profile_binding CHECK (jdbc_profile IN (1, 2, 3, 4, 5, 6));
