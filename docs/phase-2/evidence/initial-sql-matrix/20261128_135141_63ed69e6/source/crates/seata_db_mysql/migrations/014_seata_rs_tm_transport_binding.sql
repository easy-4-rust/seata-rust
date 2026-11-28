-- 保留历史明文版本 1，TLS 来源使用版本 2，旧恢复器的版本门禁继续拒绝它。
ALTER TABLE seata_rs_tm_launcher_v1
    DROP CHECK chk_tm_launcher_version,
    ADD COLUMN transport_binding BINARY(32) NULL,
    ADD CONSTRAINT chk_tm_launcher_version CHECK (schema_version IN (1, 2)),
    ADD CONSTRAINT chk_tm_launcher_transport_binding CHECK (
        (schema_version = 1 AND transport_binding IS NULL)
        OR (schema_version = 2 AND transport_binding IS NOT NULL)
    );
