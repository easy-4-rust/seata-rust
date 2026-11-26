// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"seata.apache.org/seata-go/v2/pkg/discovery"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	"seata.apache.org/seata-go/v2/pkg/remoting/loadbalance"
	client "seata.apache.org/seata-go/v2/pkg/remoting/processor/client"
	"seata.apache.org/seata-go/v2/pkg/util/log"
	"time"
)

func check(ok bool, label string) {
	if !ok {
		panic(label)
	}
}
func await(directory, name string) {
	deadline := time.Now().Add(35 * time.Second)
	for {
		if _, e := os.Stat(filepath.Join(directory, name)); e == nil {
			return
		}
		if time.Now().After(deadline) {
			panic("owned marker timeout: " + name)
		}
		time.Sleep(10 * time.Millisecond)
	}
}
func mark(directory, name string) {
	if e := os.WriteFile(filepath.Join(directory, name), []byte("owned SDK observation\n"), 0600); e != nil {
		panic(e)
	}
}
func call(body interface{}) interface{} {
	v, e := getty.GetGettyRemotingClient().SendSyncRequest(body)
	if e != nil {
		panic(e)
	}
	return v
}
func begin(name string) message.GlobalBeginRequest {
	return message.GlobalBeginRequest{Timeout: 120 * time.Second, TransactionName: name}
}
func status(xid string) message.GlobalStatusRequest {
	return message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}
}
func requireStatus(xid string, want message.GlobalStatus) {
	r := call(status(xid)).(message.GlobalStatusResponse)
	check(r.ResultCode == 1 && r.GlobalStatus == want, "authoritative TC status differs")
}
func awaitCommitFinished(xid string) []int {
	deadline := time.Now().Add(15 * time.Second)
	var history []int
	for {
		r := call(status(xid)).(message.GlobalStatusResponse)
		check(r.ResultCode == 1, "TC commit status query failed")
		history = append(history, int(r.GlobalStatus))
		if r.GlobalStatus == 15 {
			return history
		}
		check((r.GlobalStatus == 8 || r.GlobalStatus == 9) && time.Now().Before(deadline), "unexpected or stuck TC commit state")
		time.Sleep(100 * time.Millisecond)
	}
}
func rollback(xid string) {
	r := call(message.GlobalRollbackRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: xid}}).(message.GlobalRollbackResponse)
	check(r.ResultCode == 1 && r.GlobalStatus == 11, "original XID rollback failed")
}
func observedXid(directory, label string) string {
	await(directory, label+".json")
	data, e := os.ReadFile(filepath.Join(directory, label+".json"))
	if e != nil {
		panic(e)
	}
	var proof struct {
		Xid string `json:"xid"`
	}
	if e = json.Unmarshal(data, &proof); e != nil {
		panic(e)
	}
	check(proof.Xid != "", "test-only TC observer XID absent")
	return proof.Xid
}
func unknown(label string, body interface{}, deadline bool) map[string]interface{} {
	start := time.Now()
	value, e := getty.GetGettyRemotingClient().SendSyncRequest(body)
	elapsed := time.Since(start)
	var transport *getty.JavaV1TransportError
	check(value == nil && errors.As(e, &transport) && transport.Outcome() == "UnknownOutcome" && transport.RequestID() > 0, "operation did not return typed unknown: "+label)
	check(errors.Is(e, context.DeadlineExceeded) == deadline, "wrong uncertainty cause: "+label)
	if deadline {
		check(elapsed >= 19*time.Second && elapsed < 30*time.Second, "natural20s deadline was shortened or missed")
	}
	check(getty.GetGettyRemotingClient().GetMessageFuture(transport.RequestID()) == nil, "completed unknown future leaked")
	fmt.Println("INFLIGHT_UNKNOWN " + label)
	return map[string]interface{}{"label": label, "id": transport.RequestID(), "outcome": transport.Outcome(), "deadline": deadline, "elapsed_ms": elapsed.Milliseconds()}
}
func main() {
	check(len(os.Args) == 2, "one owned D:/E: runtime required")
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
	e = discovery.InitRegistryWithError(&discovery.ServiceConfig{VgroupMapping: map[string]string{"inflight": "owned"}, Grouplist: map[string]string{"owned": "127.0.0.1:28092"}}, &discovery.RegistryConfig{Type: "file"})
	if e != nil {
		panic(e)
	}
	var cfg config.Config
	flags := flag.NewFlagSet("inflight", flag.ContinueOnError)
	cfg.RegisterFlagsWithPrefix("getty", flags)
	if e = flags.Parse([]string{"-getty.java-v1-profile=true"}); e != nil {
		panic(e)
	}
	cfg.ReconnectInterval = int(50 * time.Millisecond)
	cfg.SessionConfig.CronPeriod = 250 * time.Millisecond
	cfg.SessionConfig.MaxMsgLen = 8 * 1024 * 1024
	cfg.SessionConfig.TCPReadTimeout = 5 * time.Second
	cfg.SessionConfig.WaitTimeout = 100 * time.Millisecond
	getty.InitGetty(&cfg, &config.SeataConfig{ApplicationID: "java-v1-inflight", TxServiceGroup: "inflight"})
	await(directory, "registered1.ready")
	observations := []map[string]interface{}{unknown("begin-close", begin("inflight-begin-close"), false)}
	await(directory, "registered2.ready")
	a := observedXid(directory, "begin-close")
	requireStatus(a, 1)
	rollback(a)
	requireStatus(a, 15)
	b := call(begin("inflight-commit-source")).(message.GlobalBeginResponse)
	check(b.ResultCode == 1 && b.Xid != "", "commit source begin failed")
	observations = append(observations, unknown("commit-close", message.GlobalCommitRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: b.Xid}}, false))
	await(directory, "registered3.ready")
	commitHistory := awaitCommitFinished(b.Xid)
	c := call(begin("inflight-rollback-source")).(message.GlobalBeginResponse)
	check(c.ResultCode == 1 && c.Xid != "", "rollback source begin failed")
	mark(directory, "arm-rollback-close.ready")
	observations = append(observations, unknown("rollback-close", message.GlobalRollbackRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: c.Xid}}, false))
	await(directory, "registered4.ready")
	requireStatus(c.Xid, 15)
	observations = append(observations, unknown("begin-timeout", begin("inflight-begin-timeout"), true))
	mark(directory, "timeout-observed.ready")
	await(directory, "late-timeout.ready")
	d := observedXid(directory, "begin-timeout")
	requireStatus(d, 1)
	rollback(d)
	requireStatus(d, 15)
	for id := int32(1); id <= 128; id++ {
		check(getty.GetGettyRemotingClient().GetMessageFuture(id) == nil && getty.GetGettyRemotingClient().GetMergedMessage(id) == nil, "owned future/cache leaked")
	}
	await(directory, "heartbeat4.ready")
	mark(directory, "sdk-finished.ready")
	proof, e := json.Marshal(map[string]interface{}{"real_getty_loop": true, "unknown": observations, "xids": []string{a, b.Xid, c.Xid, d}, "connections": 4, "natural_timeout_seconds": 20, "known_status_queries": 5 + len(commitHistory), "commit_statuses": commitHistory, "transaction_resends": 0, "owned_future_ids_checked": 128, "owned_futures_remaining": 0, "begin_recovery_scope": "XID from test-only TC observer; not general SDK idempotent Begin recovery"})
	if e != nil {
		panic(e)
	}
	fmt.Println("INFLIGHT_LIVE_PASS " + string(proof))
}
