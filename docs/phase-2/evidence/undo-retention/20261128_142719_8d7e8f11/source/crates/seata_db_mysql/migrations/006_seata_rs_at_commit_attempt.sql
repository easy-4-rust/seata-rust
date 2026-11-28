-- 显式扩展迁移；停写并排空旧写入器后应用，不改写原 Undo 与既有恢复表。
-- 原成功回执先于物理业务 COMMIT 独立持久化；此表不授权重新登记或重放业务。
CREATE TABLE seata_rs_at_commit_attempt (
    schema_version SMALLINT UNSIGNED NOT NULL,
    xid VARBINARY(128) NOT NULL,
    branch_id BIGINT NOT NULL,
    resource_id VARBINARY(512) NOT NULL,
    application_data MEDIUMBLOB NULL,
    lock_key MEDIUMBLOB NULL,
    jdbc_profile TINYINT UNSIGNED NOT NULL,
    response_result_code TINYINT UNSIGNED NOT NULL,
    response_exception_code TINYINT UNSIGNED NOT NULL,
    attempt_state TINYINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    modified_at DATETIME(6) NOT NULL,
    PRIMARY KEY (xid, branch_id)
) ENGINE=InnoDB;
