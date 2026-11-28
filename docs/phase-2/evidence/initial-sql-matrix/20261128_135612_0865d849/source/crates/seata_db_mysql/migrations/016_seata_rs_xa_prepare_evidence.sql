-- 显式部署原 XA 生产者证据与 Prepared 回滚结束意图；不是事务内提交见证。
CREATE TABLE seata_rs_xa_prepare_evidence_v1 (
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
