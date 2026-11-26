package org.apache.seata.rm.datasource.undo;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Objects;
import org.apache.seata.rm.datasource.DataCompareUtils;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;

/** 观察固定 H2 输入和原 Fastjson 断言的实际值。 */
public final class FastjsonUndoH2Observer {
    private FastjsonUndoH2Observer() {}

    public static void input(String method, BranchUndoLog branch) {
        System.out.printf("SEATA_FASTJSON_H2_INPUT\tBRANCH\t%s\t%s\t%d%n", method, branch.getXid(), branch.getBranchId());
        for (SQLUndoLog log : branch.getSqlUndoLogs()) {
            System.out.printf("SEATA_FASTJSON_H2_INPUT\tLOG\t%s\t%s\t%s%n", method, log.getSqlType(), log.getTableName());
            for (TableRecords image : List.of(log.getBeforeImage(), log.getAfterImage())) {
                System.out.printf("SEATA_FASTJSON_H2_INPUT\tIMAGE\t%s\t%s%n", method, image.getTableName());
                for (Row row : image.getRows()) {
                    List<Field> fields = row.getFields();
                    if (fields.size() != 2) {
                        throw new AssertionError("Original H2 row should contain ID and NAME");
                    }
                    Field id = fields.get(0);
                    Field name = fields.get(1);
                    if (!(id.getValue() instanceof Integer) || !(name.getValue() instanceof String)) {
                        throw new AssertionError("Original H2 types changed");
                    }
                    System.out.printf("SEATA_FASTJSON_H2_INPUT\tROW\t%s\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%s%n",
                            method, id.getName(), id.getType(), id.getKeyType(), id.getValue(),
                            name.getName(), name.getType(), name.getKeyType(), name.getValue());
                }
            }
        }
    }

    public static void roundtrip(byte[] bytes, BranchUndoLog before, BranchUndoLog after) {
        StringBuilder trace = new StringBuilder();
        trace.append("SEATA_FASTJSON_H2_ROUNDTRIP\t").append(Objects.nonNull(bytes))
                .append('\t').append(after.getBranchId()).append('\t').append(after.getXid())
                .append('\t').append(after.getSqlUndoLogs().size());
        for (int index = 0; index < before.getSqlUndoLogs().size(); index++) {
            SQLUndoLog src = before.getSqlUndoLogs().get(index);
            SQLUndoLog dst = after.getSqlUndoLogs().get(index);
            trace.append('\t').append(dst.getSqlType()).append('\t').append(dst.getTableName())
                    .append('\t').append(DataCompareUtils.isRecordsEquals(src.getBeforeImage(), dst.getBeforeImage()).getResult())
                    .append('\t').append(DataCompareUtils.isRecordsEquals(src.getAfterImage(), dst.getAfterImage()).getResult());
        }
        trace.append('\t').append(new String(bytes, StandardCharsets.UTF_8));
        System.out.println(trace);
    }

    public static void performance(int iterations, byte[] last, long lastBranchId) {
        System.out.printf("SEATA_FASTJSON_H2_PERFORMANCE\t%d\t%b\t%d\t%s%n", iterations,
                Objects.nonNull(last), lastBranchId, new String(last, StandardCharsets.UTF_8));
    }
}
