// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"seata.apache.org/seata-go/v2/pkg/discovery"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	"seata.apache.org/seata-go/v2/pkg/remoting/loadbalance"
	client "seata.apache.org/seata-go/v2/pkg/remoting/processor/client"
	"seata.apache.org/seata-go/v2/pkg/util/log"
)

func check(ok bool, label string) {
	if !ok {
		panic(label)
	}
}
func await(path string) {
	deadline := time.Now().Add(12 * time.Second)
	for {
		if _, e := os.Stat(path); e == nil {
			return
		}
		if time.Now().After(deadline) {
			panic("timed out waiting for owned marker: " + filepath.Base(path))
		}
		time.Sleep(20 * time.Millisecond)
	}
}
func call(body interface{}) interface{} {
	value, e := getty.GetGettyRemotingClient().SendSyncRequest(body)
	if e != nil {
		panic(e)
	}
	return value
}
func status(xid string) message.GlobalStatusRequest {
	return message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}
}
func main() {
	check(len(os.Args) == 2, "one owned D:/E: runtime directory required")
	directory, e := filepath.Abs(os.Args[1])
	if e != nil {
		panic(e)
	}
	volume := filepath.VolumeName(directory)
	check(volume == "D:" || volume == "E:", "runtime must stay on D:/E:")
	log.InitWithOption(filepath.Join(directory, "sdk.log"), log.InfoLevel)
	config.InitTransportConfig(&config.TransportConfig{Protocol: "seata"})
	loadbalance.InitLoadBalanceConfig(loadbalance.Config{Type: "RandomLoadBalance", VirtualNodes: 10})
	client.RegisterProcessor()
	if e := discovery.InitRegistryWithError(&discovery.ServiceConfig{VgroupMapping: map[string]string{"sdk_profile": "sdk_cluster"}, Grouplist: map[string]string{"sdk_cluster": "127.0.0.1:28092"}}, &discovery.RegistryConfig{Type: "file"}); e != nil {
		panic(e)
	}
	var cfg config.Config
	flags := flag.NewFlagSet("sdk", flag.ContinueOnError)
	cfg.RegisterFlagsWithPrefix("getty", flags)
	if e := flags.Parse([]string{"-getty.java-v1-profile=true"}); e != nil {
		panic(e)
	}
	cfg.ReconnectInterval = int(50 * time.Millisecond)
	cfg.SessionConfig.CronPeriod = 250 * time.Millisecond
	cfg.SessionConfig.MaxMsgLen = 8 * 1024 * 1024
	cfg.SessionConfig.TCPReadTimeout = time.Second
	cfg.SessionConfig.WaitTimeout = 100 * time.Millisecond
	getty.InitGetty(&cfg, &config.SeataConfig{ApplicationID: "java-v1-sdk-profile", TxServiceGroup: "sdk_profile"})
	// 外部配置随后改变，真实重连仍须使用初始化时捕获的选择。
	cfg.JavaV1Profile = false
	await(filepath.Join(directory, "registered1.ready"))
	batch := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{
		message.GlobalBeginRequest{Timeout: 120 * time.Second, TransactionName: "sdk-begin-a"},
		status("127.0.0.1:28091:1"),
		message.GlobalBeginRequest{Timeout: 121 * time.Second, TransactionName: "sdk-begin-b"},
	}, MsgIds: []int32{41, -42, 43}}
	reply := call(batch).(message.MergeResultMessage)
	check(len(reply.Msgs) == 3, "heterogeneous batch count differs")
	first := reply.Msgs[0].(message.GlobalBeginResponse)
	middle := reply.Msgs[1].(message.GlobalStatusResponse)
	last := reply.Msgs[2].(message.GlobalBeginResponse)
	check(first.ResultCode == 1 && last.ResultCode == 1 && middle.ResultCode == 1 && middle.GlobalStatus == 15, "merged fields differ")
	xids := []string{first.Xid, last.Xid}
	check(xids[0] != "" && xids[0] != xids[1], "original XIDs not distinct")
	for _, xid := range xids {
		r := call(status(xid)).(message.GlobalStatusResponse)
		check(r.ResultCode == 1 && r.GlobalStatus == 1, "original XID not Begin")
	}
	await(filepath.Join(directory, "heartbeat1.ready"))
	if e := os.WriteFile(filepath.Join(directory, "disconnect.request"), []byte("owned first connection only\n"), 0600); e != nil {
		panic(e)
	}
	await(filepath.Join(directory, "registered2.ready"))
	for _, xid := range xids {
		r := call(status(xid)).(message.GlobalStatusResponse)
		check(r.ResultCode == 1 && r.GlobalStatus == 1, "original XID lost after reconnect")
	}
	// 第2/3批由代理保留真实TC结果后分别注入短结果、错误子类型；调用方不能误判为业务拒绝。
	for _, label := range []string{"short-result", "wrong-child-type"} {
		request := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{status(xids[0]), status(xids[1])}}
		_, e := getty.GetGettyRemotingClient().SendSyncRequest(request)
		var unknown *getty.JavaV1UnknownReplyError
		check(errors.As(e, &unknown), "inconsistent batch did not return typed unknown outcome: "+label)
		fmt.Printf("SDK_UNKNOWN label=%s reason=%s\n", label, unknown.Reason)
		if e := os.WriteFile(filepath.Join(directory, "unknown-"+label+".ready"), []byte("typed unknown observed\n"), 0600); e != nil {
			panic(e)
		}
	}
	// 第4批代理先发错误外层ID再发原响应；不能完成其他future。
	valid := call(message.MergedWarpMessage{Msgs: []message.MessageTypeAware{status(xids[0]), status(xids[1])}}).(message.MergeResultMessage)
	check(len(valid.Msgs) == 2, "valid reply after foreign ID was lost")
	for _, child := range valid.Msgs {
		check(child.(message.GlobalStatusResponse).GlobalStatus == 1, "unknown outcomes triggered transaction resend/state change")
	}
	// 三个普通查询分别收到kind0、kind2和合法type4响应，然后才是实际TC原响应。
	for _, label := range []string{"request-envelope", "oneway-envelope", "branch-reply-type"} {
		_, e := getty.GetGettyRemotingClient().SendSyncRequest(status(xids[0]))
		var unknown *getty.JavaV1UnknownReplyError
		check(errors.As(e, &unknown), "invalid ordinary reply did not return typed unknown: "+label)
		fmt.Printf("SDK_UNKNOWN label=%s reason=%s\n", label, unknown.Reason)
		if e := os.WriteFile(filepath.Join(directory, "unknown-"+label+".ready"), []byte("typed unknown observed\n"), 0600); e != nil {
			panic(e)
		}
	}
	for _, xid := range xids {
		r := call(message.GlobalRollbackRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}).(message.GlobalRollbackResponse)
		check(r.ResultCode == 1 && r.GlobalStatus == 11, "original transaction rollback failed")
	}
	for _, xid := range xids {
		r := call(status(xid)).(message.GlobalStatusResponse)
		check(r.ResultCode == 1 && r.GlobalStatus == 15, "original transaction not Finished")
	}
	await(filepath.Join(directory, "heartbeat2.ready"))
	deadline := time.Now().Add(time.Second)
	for {
		clean := true
		for id := int32(1); id <= 128; id++ {
			if getty.GetGettyRemotingClient().GetMessageFuture(id) != nil || getty.GetGettyRemotingClient().GetMergedMessage(id) != nil {
				clean = false
			}
		}
		if clean {
			break
		}
		if time.Now().After(deadline) {
			panic("owned SDK future/correlation leaked")
		}
		time.Sleep(10 * time.Millisecond)
	}
	proof, e := json.Marshal(map[string]interface{}{"real_getty_loop": true, "captured_profile_choice": true, "xids": xids, "mixed_result_order": true, "reconnects": 1, "original_begin_queries": 4, "rollbacks": 2, "finished_queries": 2, "typed_unknown_replies": 5, "foreign_outer_id_ignored": true, "transaction_resends": 0, "owned_future_ids_checked": 128, "owned_futures_remaining": 0})
	if e != nil {
		panic(e)
	}
	if e := os.WriteFile(filepath.Join(directory, "sdk-finished.ready"), []byte("all assertions and future cleanup passed\n"), 0600); e != nil {
		panic(e)
	}
	fmt.Println("SDK_LIVE_PASS " + string(proof))
}
