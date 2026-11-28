-- 由部署管理员在 014 后执行；读取角色的整表 SELECT 已覆盖新字段。
-- 只允许创建来源时插入绑定，写角色与接管角色均不获得修改绑定的权限。
GRANT INSERT (transport_binding) ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_writer';
