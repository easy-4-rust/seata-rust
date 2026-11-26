/* Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See NOTICE. Licensed under Apache-2.0:
 * http://www.apache.org/licenses/LICENSE-2.0 . Provided on an AS IS BASIS. */
package org.apache.seata.server.session;

import org.apache.seata.common.holder.ObjectHolder;
import org.springframework.core.env.MapPropertySource;
import org.springframework.core.env.StandardEnvironment;
import org.springframework.util.StringUtils;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.apache.seata.common.Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT;

/** 为原JAR独立FILE写入进程提供真实Spring Environment，不替换原存储管理器。 */
public final class FileRetentionSeedBootstrap {
    private FileRetentionSeedBootstrap() {}

    /** 仅接受seed、独立D/E目录和状态码，再调用原持久化夹具写真实记录。 */
    public static void main(String[] args) throws Exception {
        if (args.length != 3 || !"seed".equals(args[0])) {
            throw new IllegalArgumentException("Expected seed, owned D/E directory, status");
        }
        Path directory = Paths.get(args[1]).toAbsolutePath().normalize();
        if (!directory.toString().matches("^[DE]:[/\\\\].*")) {
            throw new IllegalArgumentException("Only owned D/E FILE directories are allowed");
        }
        String fixture = System.getProperty("seata.it.fixture.config");
        if (!StringUtils.hasText(fixture) || !fixture.matches("^[DE]:[/\\\\].*")) {
            throw new IllegalArgumentException("An owned D/E file configuration is required");
        }
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("seata.config.type", "file");
        properties.put("seata.config.file.name", "file:" + fixture.replace('\\', '/'));
        properties.put("seata.store.mode", "file");
        properties.put("seata.store.session.mode", "file");
        properties.put("seata.store.lock.mode", "file");
        properties.put("seata.store.file.dir", directory.toString());
        StandardEnvironment environment = new StandardEnvironment();
        environment.getPropertySources().addFirst(new MapPropertySource("owned-file-retention-writer", properties));
        ObjectHolder.INSTANCE.setObject(OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT, environment);
        FileRetryTimeoutStoreProbe.main(args);
    }
}
