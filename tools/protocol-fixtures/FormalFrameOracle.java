/* Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0. */
package org.easy4rust.seata.oracle;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufUtil;
import io.netty.buffer.Unpooled;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.core.protocol.AbstractMessage;
import org.apache.seata.core.protocol.AbstractResultMessage;
import org.apache.seata.core.protocol.RpcMessage;
import org.apache.seata.core.rpc.netty.v1.HeadMapSerializer;
import org.apache.seata.core.rpc.netty.v1.ProtocolDecoderV1;
import org.apache.seata.core.rpc.netty.v1.ProtocolEncoderV1;
import org.apache.seata.core.rpc.netty.v1.ProtocolRpcMessageV1;
import org.apache.seata.serializer.seata.SeataSerializer;
import org.apache.seata.serializer.seata.serializer.SeataSerializerV1;
import org.springframework.core.env.StandardEnvironment;

/** 使用固定原Java编码器生成完整帧，并以原解码器核对全部语义字段。 */
public final class FormalFrameOracle {
    private static final ObjectMapper JSON = new ObjectMapper();

    private static Map<String,Object> row(Object... values) {
        Map<String,Object> result = new LinkedHashMap<>();
        for (int i=0;i<values.length;i+=2) result.put((String)values[i],values[i+1]);
        return result;
    }

    /** 参数为原始载荷输入JSON和独立输出目录；返回37帧及实际生产类身份，任一断言失败退出非零。 */
    public static void main(String[] args) throws Exception {
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,new StandardEnvironment());
        System.setProperty("file.listener.enabled","false");
        System.setProperty("config.type","file");
        System.setProperty("config.file.name","file");
        System.setProperty("registry.type","file");
        Path output=Path.of(args[1]);
        SeataSerializer serializer=new SeataSerializer((byte)1);
        List<Object> records=new ArrayList<>();
        List<String> lines=new ArrayList<>();
        AbstractMessage begin=null;
        int id=0;
        for (JsonNode input:JSON.readTree(Files.readAllBytes(Path.of(args[0])))) {
            AbstractMessage body=serializer.deserialize(HexFormat.of().parseHex(input.path("serialized_hex").asText()));
            if (input.path("name").asText().equals("GlobalBeginRequest")) begin=body;
            capture(input.path("name").asText(),body,++id,(byte)(body instanceof AbstractResultMessage ? 1 : 0),
                    new LinkedHashMap<>(),records,lines);
        }
        Objects.requireNonNull(begin,"begin input required");
        for (String name:List.of("HeaderNull","HeaderEmpty","HeaderEmptyKey","HeaderOrdered","HeaderMaximum",
                "SignedMinimum","SignedMaximum","Oneway")) {
            Map<String,String> headers=new LinkedHashMap<>();
            int requestId=1;
            byte kind=0;
            switch (name) {
                case "HeaderNull":headers.put("k",null);break;
                case "HeaderEmpty":headers.put("k","");break;
                case "HeaderEmptyKey":headers.put("","");break;
                case "HeaderOrdered":headers.put("z",null);headers.put("","");headers.put("a","中文🦀");break;
                case "HeaderMaximum":headers.put("k","x".repeat(32746));break;
                case "SignedMinimum":requestId=Integer.MIN_VALUE;break;
                case "SignedMaximum":requestId=Integer.MAX_VALUE;break;
                case "Oneway":kind=2;break;
                default:throw new IllegalArgumentException(name);
            }
            capture(name,begin,requestId,kind,headers,records,lines);
        }
        if (records.size()!=37) throw new IllegalStateException("Need all37 records");
        List<Object> origins=new ArrayList<>();
        for (Class<?> type:List.of(ProtocolEncoderV1.class,ProtocolDecoderV1.class,HeadMapSerializer.class,
                RpcMessage.class,ProtocolRpcMessageV1.class,SeataSerializer.class,SeataSerializerV1.class)) {
            try (InputStream input=type.getResourceAsStream("/"+type.getName().replace('.','/')+".class")) {
                byte[] bytes=Objects.requireNonNull(input).readAllBytes();
                origins.add(row("class",type.getName(),"source",type.getProtectionDomain().getCodeSource().getLocation().toString(),
                        "sha256",HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes))));
            }
        }
        Files.write(output.resolve("java.json"),JSON.writerWithDefaultPrettyPrinter().writeValueAsBytes(row(
                "execution_kind","actual_original_Java_encoder_and_decoder_without_JUnit_engine", "frames",records,"origins",origins)));
        Files.writeString(output.resolve("java.tsv"),String.join("\n",lines)+"\n");
    }

    private static void capture(String name,AbstractMessage body,int id,byte kind,Map<String,String> headers,
            List<Object> records,List<String> lines) throws Exception {
        RpcMessage rpc=new RpcMessage();
        rpc.setId(id);rpc.setMessageType(kind);rpc.setCodec((byte)1);rpc.setCompressor((byte)0);
        rpc.setHeadMap(headers);rpc.setBody(body);
        ByteBuf frame=Unpooled.buffer();
        byte[] bytes;
        try {
            new ProtocolEncoderV1().encode(rpc,frame);
            bytes=ByteBufUtil.getBytes(frame);
            RpcMessage decoded=new ProtocolDecoderV1().decodeFrame(frame);
            if (decoded.getId()!=id || decoded.getMessageType()!=kind || decoded.getCodec()!=1
                    || decoded.getCompressor()!=0 || !headers.equals(decoded.getHeadMap()) || frame.isReadable()
                    || !JSON.valueToTree(body).equals(JSON.valueToTree(decoded.getBody()))) {
                throw new IllegalStateException("Original Java field mismatch: "+name);
            }
        } finally { frame.release(); }
        int head=((bytes[7]&255)<<8)|(bytes[8]&255);
        String full=HexFormat.of().formatHex(bytes);
        String headHex=HexFormat.of().formatHex(bytes,16,head);
        String payload=HexFormat.of().formatHex(bytes,head,bytes.length);
        records.add(row("name",name,"id",id,"kind",kind,"headers",headers,"body",JSON.valueToTree(body),
                "frame_hex",full,"head_hex",headHex,"payload_hex",payload));
        lines.add(String.join("\t",name,Integer.toString(id),Byte.toString(kind),headHex,full,payload));
    }
}
