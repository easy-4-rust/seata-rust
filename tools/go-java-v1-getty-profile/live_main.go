// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"time"

	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
)

func check(ok bool, label string) {
	if !ok {
		panic(label)
	}
}

type peer struct {
	conn     net.Conn
	handler  *getty.RpcPackageHandler
	pending  []byte
	sent     int
	received int
}

func (p *peer) send(rpc message.RpcMessage) {
	wire, e := p.handler.Write(nil, rpc)
	if e != nil {
		panic(e)
	}
	// 真实TCP分三段写入；不重试事务，不假定一次Write发送全部字节。
	for _, part := range [][]byte{wire[:1], wire[1:15], wire[15:]} {
		for len(part) > 0 {
			n, e := p.conn.Write(part)
			if e != nil {
				panic(fmt.Errorf("outcome unknown after write: %w", e))
			}
			if n == 0 {
				panic("zero network write; outcome unknown")
			}
			part = part[n:]
		}
	}
	p.sent++
	fmt.Printf("GETTY_LIVE_SENT id=%d kind=%d frame=%s\n", rpc.ID, rpc.Type, hex.EncodeToString(wire))
}
func (p *peer) call(id int32, body interface{}) message.RpcMessage {
	if e := p.conn.SetDeadline(time.Now().Add(12 * time.Second)); e != nil {
		panic(e)
	}
	p.send(message.RpcMessage{ID: id, Type: 0, Codec: 1, Body: body})
	for {
		value, n, e := p.handler.Read(nil, p.pending)
		if e != nil {
			panic(fmt.Errorf("outcome unknown after receive: %w", e))
		}
		if value != nil {
			rpc := value.(message.RpcMessage)
			wire := append([]byte(nil), p.pending[:n]...)
			p.pending = p.pending[n:]
			p.received++
			fmt.Printf("GETTY_LIVE_RECEIVED id=%d kind=%d frame=%s\n", rpc.ID, rpc.Type, hex.EncodeToString(wire))
			if rpc.Type == 3 {
				p.send(message.RpcMessage{ID: rpc.ID, Type: 4, Codec: 1, Body: message.HeartBeatMessagePong})
				continue
			}
			check(rpc.ID == id && rpc.Type == 1, "response correlation differs")
			return rpc
		}
		check(len(p.pending) < 8*1024*1024, "receive buffer bound exceeded")
		chunk := make([]byte, 4096)
		n, e = p.conn.Read(chunk)
		if n > 0 {
			p.pending = append(p.pending, chunk[:n]...)
			continue
		}
		if e != nil {
			panic(fmt.Errorf("outcome unknown; no transaction resend: %w", e))
		}
	}
}
func main() {
	check(len(os.Args) == 2, "one loopback TC address required")
	address := os.Args[1]
	host, _, e := net.SplitHostPort(address)
	if e != nil {
		panic(e)
	}
	check(host == "127.0.0.1", "loopback only")
	codec.Init()
	profile, e := codec.NewJavaV1MessageProfile(codec.GetCodecManager())
	if e != nil {
		panic(e)
	}
	conn, e := net.DialTimeout("tcp", address, 3*time.Second)
	if e != nil {
		panic(e)
	}
	defer conn.Close()
	p := &peer{conn: conn, handler: &getty.RpcPackageHandler{PayloadCodec: profile}}
	registered := p.call(1, message.RegisterTMRequest{AbstractIdentifyRequest: message.AbstractIdentifyRequest{Version: "1.1.0", ApplicationId: "getty-profile-probe", TransactionServiceGroup: "getty_profile"}}).Body.(message.RegisterTMResponse)
	check(registered.Identified && registered.Version == "2.8.0-SNAPSHOT", "registration rejected/version differs")
	// 使用固定Go版本1.1.0，使未改动TC选择旧版60响应而非尚未支持的121批量结果。
	batch := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{
		message.GlobalBeginRequest{Timeout: 60 * time.Second, TransactionName: "getty-begin-a"},
		message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "127.0.0.1:28091:1"}},
		message.GlobalBeginRequest{Timeout: 61 * time.Second, TransactionName: "getty-begin-b"},
	}, MsgIds: []int32{1011, -1012, 1013}}
	result := p.call(2, batch).Body.(message.MergeResultMessage)
	check(len(result.Msgs) == 3, "merge result count differs")
	first := result.Msgs[0].(message.GlobalBeginResponse)
	middle := result.Msgs[1].(message.GlobalStatusResponse)
	last := result.Msgs[2].(message.GlobalBeginResponse)
	check(first.ResultCode == 1 && last.ResultCode == 1 && first.Xid != "" && last.Xid != "" && first.Xid != last.Xid, "begin did not create distinct original XIDs")
	check(middle.ResultCode == 1 && middle.GlobalStatus == 15, "heterogeneous merged result order/status differs")
	xids := []string{first.Xid, last.Xid}
	id := int32(3)
	for _, xid := range xids {
		r := p.call(id, message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}).Body.(message.GlobalStatusResponse)
		id++
		check(r.ResultCode == 1 && r.GlobalStatus == 1, "original XID not Begin")
	}
	for _, xid := range xids {
		r := p.call(id, message.GlobalRollbackRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}).Body.(message.GlobalRollbackResponse)
		id++
		check(r.ResultCode == 1 && r.GlobalStatus == 11, "rollback did not complete")
	}
	for _, xid := range xids {
		r := p.call(id, message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}).Body.(message.GlobalStatusResponse)
		id++
		check(r.ResultCode == 1 && r.GlobalStatus == 15, "original XID not Finished after rollback")
	}
	output, e := json.Marshal(map[string]interface{}{"identified": true, "java_version": registered.Version, "client_wire_version": "1.1.0", "outer_id": 2, "merge_children": 3, "mixed_result_order": true, "xids": xids, "begin_queries": 2, "rollbacks": 2, "finished_queries": 2, "sent": p.sent, "received": p.received, "transaction_resends": 0})
	if e != nil {
		panic(e)
	}
	fmt.Println("GETTY_LIVE_PASS " + string(output))
}
