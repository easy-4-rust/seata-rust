-- 当所有旧写入器及 XID 已排空后，由部署方配置原 Connector/J 的 tinyInt1isBit 映射。
-- 1 表示 Boolean，0 表示数字。旧行保留 NULL，等待显式配置。
ALTER TABLE seata_rs_at_resource_binding
    ADD COLUMN tinyint1_is_bit TINYINT UNSIGNED NULL,
    ADD CONSTRAINT ck_seata_rs_at_tinyint_profile_binding CHECK (tinyint1_is_bit IN (0, 1));
