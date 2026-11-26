-- 在目标业务 schema 安装。部署时为 CREATE PROCEDURE 指定锁定登录的专用定义者，
-- 仅为定义者授予直接全局 REFERENCES、当前 schema 的直接 TRIGGER 及本过程的 EXECUTE。
-- 运行账号仅获准直接 EXECUTE 本过程，部署账号管理过程定义和授权。
DELIMITER //
CREATE DEFINER='metadata_definer'@'%' PROCEDURE seata_rs_at_constraint_flags_v1(IN p_table VARCHAR(64) CHARACTER SET utf8mb4)
READS SQL DATA
SQL SECURITY DEFINER
BEGIN
    SHOW GRANTS FOR CURRENT_USER();
    SELECT CAST(
        EXISTS (
            SELECT 1 FROM information_schema.TRIGGERS
            WHERE TRIGGER_SCHEMA=DATABASE() AND EVENT_OBJECT_TABLE=p_table
        ) + 2 * EXISTS (
            SELECT 1 FROM information_schema.REFERENTIAL_CONSTRAINTS
            WHERE (CONSTRAINT_SCHEMA=DATABASE() AND TABLE_NAME=p_table)
               OR (UNIQUE_CONSTRAINT_SCHEMA=DATABASE() AND REFERENCED_TABLE_NAME=p_table)
        ) AS SIGNED
    ) AS seata_constraint_flags;
END//
DELIMITER ;
