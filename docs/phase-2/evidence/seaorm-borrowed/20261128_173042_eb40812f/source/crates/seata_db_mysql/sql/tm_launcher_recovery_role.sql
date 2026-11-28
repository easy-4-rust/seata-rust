-- 由部署管理员在 Launcher 数据库执行，单独授权给持有监护器的可信控制进程。
CREATE ROLE 'seata_tm_launcher_recovery';

GRANT SELECT, UPDATE (owner_instance_id, owner_epoch, owner_stop_scope, modified_at)
ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_recovery';
