-- 显式部署原 XA guard 协议登记/事务内 guard；禁止运行时迁移或补造旧分支。
CREATE TABLE seata_rs_xa_session_guard_v1 (
    xid VARBINARY(64) NOT NULL,
    branch_id BIGINT NOT NULL,
    resource_id VARBINARY(512) NOT NULL,
    application_data MEDIUMBLOB NULL,
    server_uuid VARBINARY(36) NOT NULL,
    schema_version SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    phase TINYINT UNSIGNED NOT NULL DEFAULT 0,
    completed TINYINT UNSIGNED NOT NULL DEFAULT 0,
    PRIMARY KEY (xid, branch_id)
) ENGINE=InnoDB;
