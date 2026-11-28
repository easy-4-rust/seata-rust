-- 仅供已有 Launcher 角色的部署在 010 迁移后升级，由部署管理员在 Launcher 数据库执行。
GRANT INSERT (owner_stop_scope) ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_writer';
GRANT UPDATE (owner_stop_scope) ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_recovery';
