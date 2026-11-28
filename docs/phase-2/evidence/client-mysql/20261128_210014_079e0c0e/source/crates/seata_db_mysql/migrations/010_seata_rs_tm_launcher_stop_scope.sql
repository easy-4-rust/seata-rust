-- 未配置信任域的历史来源可走本机证明，远程签名接管须匹配持久化信任域。
ALTER TABLE seata_rs_tm_launcher_v1
    ADD COLUMN owner_stop_scope BINARY(32) NULL AFTER owner_instance_id;
