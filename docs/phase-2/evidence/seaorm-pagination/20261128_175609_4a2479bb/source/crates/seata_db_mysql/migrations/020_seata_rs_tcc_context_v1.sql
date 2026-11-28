-- 显式启用动态报告时部署；每个原分支最多 65 个不可覆盖版本（含原登记版本 0）。
CREATE TABLE seata_rs_tcc_context_v1 (
    xid VARBINARY(128) NOT NULL,
    branch_id BIGINT NOT NULL,
    revision SMALLINT UNSIGNED NOT NULL,
    resource_id VARBINARY(512) NOT NULL,
    registration_data MEDIUMBLOB NOT NULL,
    application_data MEDIUMBLOB NOT NULL,
    server_uuid VARBINARY(36) NOT NULL,
    schema_version SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    checksum BINARY(32) NOT NULL,
    PRIMARY KEY (xid, branch_id, revision)
) ENGINE=InnoDB;
