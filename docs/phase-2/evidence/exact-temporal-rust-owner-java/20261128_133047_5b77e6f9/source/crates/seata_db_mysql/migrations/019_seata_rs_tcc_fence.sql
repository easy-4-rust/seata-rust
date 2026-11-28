-- Rust TCC 独立 Fence，显式部署；不修改 Java Undo/原 Fence 或 AT/XA 控制结构。
CREATE TABLE seata_rs_tcc_fence_v1 (
    xid VARBINARY(128) NOT NULL,
    branch_id BIGINT NOT NULL,
    resource_id VARBINARY(512) NOT NULL,
    application_data MEDIUMBLOB NULL,
    server_uuid VARBINARY(36) NOT NULL,
    schema_version SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    status TINYINT UNSIGNED NOT NULL,
    PRIMARY KEY (xid, branch_id)
) ENGINE=InnoDB;
