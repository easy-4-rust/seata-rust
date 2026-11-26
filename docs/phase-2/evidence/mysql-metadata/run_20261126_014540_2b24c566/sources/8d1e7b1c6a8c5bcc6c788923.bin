-- 可选迁移。写入器排空后，由部署方独立预置 slot=1 的原 JDBC 资源 ID。
-- 不在迁移中猜测端点，不从首个客户端自动登记，不改写已有恢复行与 Undo。
CREATE TABLE seata_rs_at_resource_binding (
    slot TINYINT UNSIGNED NOT NULL,
    resource_id VARBINARY(512) NOT NULL,
    PRIMARY KEY (slot),
    CONSTRAINT ck_seata_rs_at_resource_binding_slot CHECK (slot = 1)
) ENGINE=InnoDB;
