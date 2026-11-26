/* Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0. */
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.apache.seata.core.exception.TransactionExceptionCode;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.transaction.AbstractGlobalEndResponse;
import org.apache.seata.core.protocol.transaction.GlobalCommitResponse;
import org.apache.seata.core.protocol.transaction.GlobalRollbackResponse;
import org.apache.seata.core.protocol.transaction.GlobalStatusResponse;
import org.apache.seata.core.protocol.transaction.GlobalReportResponse;
import org.apache.seata.serializer.seata.SeataSerializer;
import org.apache.seata.server.session.SessionStatusValidator;

/** 原 JVM 全局状态与载荷观察器。对应 Java: GlobalStatus、SessionStatusValidator。 */
public final class GlobalStatusContractProbe {
    private static Map<String, Object> row(Object... fields) {
        Map<String, Object> out = new LinkedHashMap<>();
        for (int i = 0; i < fields.length; i += 2) out.put((String) fields[i], fields[i + 1]);
        return out;
    }

    private static AbstractGlobalEndResponse response(int type) {
        switch (type) {
            case 8: return new GlobalCommitResponse();
            case 10: return new GlobalRollbackResponse();
            case 16: return new GlobalStatusResponse();
            case 18: return new GlobalReportResponse();
            default: throw new IllegalArgumentException("Unsupported response");
        }
    }

    private static boolean rejected(int code, boolean asByte) {
        try {
            if (asByte) GlobalStatus.get((byte) code); else GlobalStatus.get(code);
            return false;
        } catch (IllegalArgumentException expected) { return true; }
    }

    /** 运行原状态方法与四种响应的序列化，将观察结果写入指定文件；参数为输出路径。 */
    public static void main(String[] args) throws Exception {
        List<Object> statuses = new ArrayList<>();
        List<Object> transitions = new ArrayList<>();
        List<Object> responses = new ArrayList<>();
        List<Object> unknown = new ArrayList<>();
        List<Object> origins = new ArrayList<>();
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        for (GlobalStatus status : GlobalStatus.values()) {
            statuses.add(row("code", status.getCode(), "name", status.name(),
                "one_phase_timeout", GlobalStatus.isOnePhaseTimeout(status),
                "two_phase_success", GlobalStatus.isTwoPhaseSuccess(status),
                "two_phase_heuristic", GlobalStatus.isTwoPhaseHeuristic(status),
                "one_phase_prepare_failed", GlobalStatus.isOnePhasePrepareFailed(status),
                "timeout_global", SessionStatusValidator.isTimeoutGlobalStatus(status),
                "timeout_rollbacking", SessionStatusValidator.isTimeoutRollbacking(status),
                "rollback_global", SessionStatusValidator.isRollbackGlobalStatus(status),
                "end_global", SessionStatusValidator.isEndGlobalStatus(status),
                "commit_global", SessionStatusValidator.isCommitGlobalStatus(status)));
            if (GlobalStatus.get(status.getCode()) != status || GlobalStatus.get((byte) status.getCode()) != status)
                throw new IllegalStateException("Status lookup changed");
            for (GlobalStatus after : GlobalStatus.values()) {
                transitions.add(row("before", status.getCode(), "after", after.getCode(),
                    "allowed", SessionStatusValidator.validateUpdateStatus(status, after)));
            }
            for (int type : new int[] {8, 10, 16, 18}) {
                for (int result : new int[] {0, 1}) {
                    AbstractGlobalEndResponse value = response(type);
                    value.setGlobalStatus(status);
                    value.setResultCode(result == 0 ? ResultCode.Failed : ResultCode.Success);
                    value.setTransactionExceptionCode(result == 0 ? TransactionExceptionCode.IO : TransactionExceptionCode.Unknown);
                    value.setMsg(result == 0 ? "状态诊断:IO" : null);
                    byte[] bytes = serializer.serialize(value);
                    AbstractGlobalEndResponse decoded = serializer.deserialize(bytes);
                    if (decoded.getGlobalStatus() != status || decoded.getResultCode() != value.getResultCode()
                        || decoded.getTransactionExceptionCode() != value.getTransactionExceptionCode()
                        || !Arrays.equals(bytes, serializer.serialize(decoded)))
                        throw new IllegalStateException("Java field/byte round trip changed");
                    responses.add(row("type", type, "status", status.getCode(), "result", result,
                        "error", decoded.getTransactionExceptionCode().ordinal(),
                        "msg", result == 0 ? decoded.getMsg() : "", "hex", java.util.HexFormat.of().formatHex(bytes)));
                }
            }
        }
        for (int code = 21; code <= 255; code++)
            unknown.add(row("code", code, "int_rejected", rejected(code, false), "byte_rejected", rejected(code, true)));
        for (Class<?> type : new Class<?>[] {GlobalStatus.class, SessionStatusValidator.class, SeataSerializer.class,
                GlobalCommitResponse.class, GlobalRollbackResponse.class, GlobalStatusResponse.class, GlobalReportResponse.class})
            origins.add(origin(type));
        for (String name : new String[] {"org.apache.seata.serializer.seata.SeataSerializer$SeataSerializerV1",
                "org.apache.seata.serializer.seata.MessageCodecFactory",
                "org.apache.seata.serializer.seata.protocol.AbstractResultMessageCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.AbstractTransactionResponseCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.AbstractGlobalEndResponseCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.GlobalCommitResponseCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.GlobalRollbackResponseCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.GlobalStatusResponseCodec",
                "org.apache.seata.serializer.seata.protocol.transaction.GlobalReportResponseCodec"})
            origins.add(origin(Class.forName(name)));
        ObjectMapper json = new ObjectMapper();
        Files.write(Path.of(args[0]), json.writerWithDefaultPrettyPrinter().writeValueAsBytes(row(
            "statuses", statuses, "transitions", transitions, "responses", responses, "unknown", unknown, "origins", origins)));
    }

    private static Map<String, Object> origin(Class<?> type) {
        return row("class", type.getName(), "location", type.getProtectionDomain().getCodeSource().getLocation().toString());
    }
}
