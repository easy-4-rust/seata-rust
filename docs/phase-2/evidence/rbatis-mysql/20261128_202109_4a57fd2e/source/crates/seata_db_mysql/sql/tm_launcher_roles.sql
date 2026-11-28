-- 由部署管理员在执行 008 至 010 后，于选定的 Launcher 数据库执行。
-- 角色名称在服务器上共享。已存在时停止并检查授权，避免继承其他数据库的权限。
CREATE ROLE 'seata_tm_launcher_reader', 'seata_tm_launcher_writer';

GRANT SELECT ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_reader';

GRANT SELECT,
    INSERT (schema_version, xid_digest, xid, tc_endpoint, application_id, service_group,
            begin_exception_code, begin_extra_data, business_intent, owner_instance_id, owner_stop_scope,
            deadline_at, created_at, modified_at),
    UPDATE (end_operation, send_started, modified_at)
ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_writer';
