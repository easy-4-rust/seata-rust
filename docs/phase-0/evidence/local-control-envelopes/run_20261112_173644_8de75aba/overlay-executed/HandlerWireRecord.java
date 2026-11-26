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
package org.apache.seata.serializer.seata;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.util.LinkedHashMap;
import java.util.Map;

/** 只输出验证事件，不读取输入数据或替代真实 Java 编解码器；供跨语言测试提取。 */
public final class HandlerWireRecord {
    private static final ObjectMapper JSON = new ObjectMapper();

    private HandlerWireRecord() {}

    /** 输入已验证的真实载荷和独立字段预期，输出一个带用例身份的 JSON 行；失败时抛出异常。 */
    public static void emit(
            String caseId,
            String operation,
            byte[] payload,
            int result,
            int error,
            String errorName,
            String msg,
            int status,
            String xid,
            long branchId,
            boolean lockable,
            String version)
            throws Exception {
        Map<String, Object> fields = new LinkedHashMap<>();
        fields.put("format_version", 1);
        fields.put("case_id", caseId);
        fields.put("operation", operation);
        fields.put("payload_hex", hex(payload));
        fields.put("result_code", result);
        fields.put("exception_code", error);
        fields.put("exception_name", errorName);
        fields.put("msg", msg);
        fields.put("global_status", status);
        fields.put("xid", xid);
        fields.put("branch_id", branchId);
        fields.put("lockable", lockable);
        fields.put("version", version);
        System.out.println("SEATA_HANDLER_WIRE " + JSON.writeValueAsString(fields));
    }

    /** 输入非空载荷，返回小写十六进制；不进行语义判断。 */
    public static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte value : bytes) {
            out.append(Character.forDigit((value & 255) >>> 4, 16));
            out.append(Character.forDigit(value & 15, 16));
        }
        return out.toString();
    }

    /** 输入不可编码用例身份与问题编号，输出独立诊断事件；不伪造响应字节。 */
    public static void unencodable(String caseId, String issue) throws Exception {
        Map<String, Object> fields = new LinkedHashMap<>();
        fields.put("format_version", 1);
        fields.put("case_id", caseId);
        fields.put("issue", issue);
        System.out.println("SEATA_HANDLER_UNENCODABLE " + JSON.writeValueAsString(fields));
    }
}
