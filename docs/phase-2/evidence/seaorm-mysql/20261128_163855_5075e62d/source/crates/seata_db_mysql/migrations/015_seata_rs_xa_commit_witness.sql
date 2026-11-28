-- 显式部署 XA 同事务提交见证；仅原 XA 提交后可见，不从缺行推断回滚。
CREATE TABLE seata_rs_xa_commit_witness_v1 (
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
