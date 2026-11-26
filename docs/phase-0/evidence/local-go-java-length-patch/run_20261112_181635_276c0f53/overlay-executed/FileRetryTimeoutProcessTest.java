/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package org.apache.seata.server.session;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 通过两个独立 JVM 验证真实 FILE 写入及读取，避免进程内 sessionMap 充当恢复证据。 */
class FileRetryTimeoutProcessTest {
    @TempDir
    Path temporary;

    @ParameterizedTest
    @ValueSource(ints = {1, 16, 17})
    void aFreshJvmReadsActualPersistedStatusAndBranch(int status) throws Exception {
        Path store = temporary.resolve("store with spaces " + status);
        String seeded = child("seed", store, status, 0);
        assertTrue(seeded.contains("SEEDED status=" + status + " branches=1"), seeded);
        Path data = store.resolve("root.data");
        assertTrue(Files.size(data) > 0);
        String restored = child("inspect", store, status, 0);
        if (status == 17) {
            assertTrue(restored.contains("RESTORED sessions=0 branches=0 status=absent"), restored);
        } else {
            assertTrue(restored.contains("RESTORED sessions=1 branches=1 status=" + status), restored);
            assertTrue(
                    restored.contains(status == 16 ? "RELOAD_SWITCH=UNHANDLED_16" : "RELOAD_SWITCH=ACTIVE_BEGIN"),
                    restored);
        }
    }

    @Test
    void seedCannotOverwriteExistingStore() throws Exception {
        Path store = temporary.resolve("existing");
        child("seed", store, 1, 0);
        assertTrue(Files.isRegularFile(store.resolve("root.data")), "Seed produced no persisted store");
        byte[] before = Files.readAllBytes(store.resolve("root.data"));
        String rejected = child("seed", store, 16, 1);
        assertTrue(rejected.contains("Refusing existing store directory"), rejected);
        assertTrue(Arrays.equals(before, Files.readAllBytes(store.resolve("root.data"))));
    }

    @Test
    void inspectCannotCreateAnEmptyStoreAndCallItRestored() throws Exception {
        Path missing = temporary.resolve("missing");
        String rejected = child("inspect", missing, 17, 1);
        assertTrue(rejected.contains("Missing persisted store"), rejected);
        assertFalse(Files.exists(missing));
    }

    private String child(String mode, Path directory, int status, int expectedExit) throws Exception {
        Path arguments = temporary.resolve(UUID.randomUUID() + ".args");
        Path log = temporary.resolve(UUID.randomUUID() + ".log");
        String classpath = System.getProperty("surefire.test.class.path", System.getProperty("java.class.path"));
        String content = quote("-Djava.io.tmpdir=" + System.getProperty("java.io.tmpdir")) + "\n"
                + quote("-Duser.home=" + temporary) + "\n-cp\n"
                + quote(classpath) + "\n" + FileRetryTimeoutStoreProbe.class.getName() + "\n"
                + mode + "\n" + quote(directory.toString()) + "\n" + status + "\n";
        Files.write(arguments, content.getBytes(StandardCharsets.UTF_8), StandardOpenOption.CREATE_NEW);
        Process process = new ProcessBuilder(
                        Paths.get(System.getProperty("java.home"), "bin", "java")
                                .toString(),
                        "@" + arguments)
                .redirectErrorStream(true)
                .redirectOutput(log.toFile())
                .start();
        boolean exited = process.waitFor(30, TimeUnit.SECONDS);
        if (!exited) {
            process.destroyForcibly();
            assertTrue(process.waitFor(10, TimeUnit.SECONDS), "Owned child failed to exit");
        }
        String output = new String(Files.readAllBytes(log), StandardCharsets.UTF_8);
        assertTrue(exited, output);
        assertEquals(expectedExit, process.exitValue(), output);
        return output;
    }

    private String quote(String argument) {
        return "\"" + argument.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }
}
