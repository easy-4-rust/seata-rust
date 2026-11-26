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
import jdk.jfr.consumer.RecordedClass;
import jdk.jfr.consumer.RecordedEvent;
import jdk.jfr.consumer.RecordedFrame;
import jdk.jfr.consumer.RecordedMethod;
import jdk.jfr.consumer.RecordedStackTrace;
import jdk.jfr.consumer.RecordingFile;
import java.nio.file.Path;
import java.util.Objects;

/** 使用原 JDK 流式读取完整异常调用栈，避免 jfr print 默认五帧截断及无关类加载事件膨胀。 */
public final class ReadRaftExceptions {
    private static String quote(String value) {
        if (Objects.isNull(value)) return "null";
        StringBuilder out = new StringBuilder("\"");
        for (int i=0; i<value.length(); i++) {
            char c=value.charAt(i);
            if (c=='\\' || c=='\"') out.append('\\').append(c);
            else if (c<32) out.append(String.format("\\u%04x",(int)c));
            else out.append(c);
        }
        return out.append('"').toString();
    }

    /** 输入一个 D/E JFR 文件，输出真正异常的标准字段和全部已记录帧，不执行事务代码。 */
    public static void main(String[] args) throws Exception {
        if (args.length!=1) throw new IllegalArgumentException("Expected one owned JFR path");
        Path source=Path.of(args[0]).toRealPath();
        if (!source.toString().matches("^[DE]:[\\\\/].*")) throw new IllegalArgumentException("JFR data must remain on D/E");
        System.out.print("{\"recording\":{\"events\":[");
        boolean firstEvent=true;
        try (RecordingFile recording=new RecordingFile(source)) {
            while (recording.hasMoreEvents()) {
                RecordedEvent event=recording.readEvent();
                if (!"jdk.JavaExceptionThrow".equals(event.getEventType().getName())) continue;
                RecordedClass thrown=event.getClass("thrownClass");
                if (Objects.isNull(thrown) || !"org.apache.seata.common.exception.ShouldNeverHappenException".equals(thrown.getName())) continue;
                if (!firstEvent) System.out.print(',');
                firstEvent=false;
                System.out.print("{\"type\":\"jdk.JavaExceptionThrow\",\"values\":{\"startTime\":"+quote(event.getStartTime().toString())
                        +",\"thrownClass\":{\"name\":"+quote(thrown.getName())+"},\"message\":"+quote(event.getString("message")));
                RecordedStackTrace trace=event.getStackTrace();
                System.out.print(",\"stackTrace\":{\"frames\":[");
                boolean firstFrame=true;
                if (Objects.nonNull(trace)) {
                    for (RecordedFrame frame:trace.getFrames()) {
                        if (!firstFrame) System.out.print(',');
                        firstFrame=false;
                        RecordedMethod method=frame.getMethod();
                        System.out.print("{\"method\":{\"type\":{\"name\":"+quote(method.getType().getName())+"},\"name\":"+quote(method.getName())+"}}");
                    }
                }
                System.out.print("]}}}");
            }
        }
        System.out.println("]}}");
    }
}
