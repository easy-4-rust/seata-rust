-- 显式部署后启用受管理 XA；保留登记防重放行，不与原 AT/Undo 表混用。
CREATE TABLE seata_rs_xa_branch_v1 (
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
