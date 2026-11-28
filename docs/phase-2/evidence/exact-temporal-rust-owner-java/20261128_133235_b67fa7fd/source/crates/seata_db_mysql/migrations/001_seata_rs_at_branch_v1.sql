-- 显式扩展迁移；不得在业务事务中执行，保留原 undo_log 不变。
-- 部署前确认不存在同名表；不使用 IF NOT EXISTS 掩盖不兼容结构。
CREATE TABLE seata_rs_at_branch_v1 (
    schema_version SMALLINT UNSIGNED NOT NULL,
    xid VARBINARY(128) NOT NULL,
    branch_id BIGINT NOT NULL,
    resource_id VARBINARY(32767) NOT NULL,
    application_data MEDIUMBLOB NULL,
    lock_key MEDIUMBLOB NULL,
    jdbc_profile TINYINT UNSIGNED NOT NULL,
    phase TINYINT UNSIGNED NOT NULL,
    terminal TINYINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    modified_at DATETIME(6) NOT NULL,
    PRIMARY KEY (xid, branch_id)
) ENGINE=InnoDB;
