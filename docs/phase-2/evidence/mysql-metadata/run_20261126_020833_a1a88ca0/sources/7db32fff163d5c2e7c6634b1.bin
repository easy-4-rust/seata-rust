-- 为现有 008 来源增补实际 TM 实例身份；旧行无法证明签发进程，保留 NULL 并禁止结束 CAS。
-- 新 Begin 必须写入操作系统随机生成的 32 字节 ID；只有已绑定实例的来源可用于后续受信接管。
ALTER TABLE seata_rs_tm_launcher_v1
    ADD COLUMN owner_instance_id BINARY(32) NULL AFTER owner_epoch;
