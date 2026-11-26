// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"fmt"
	getty "github.com/apache/dubbo-getty"
	"net"
	"seata.apache.org/seata-go/v2/pkg/discovery"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"strconv"
	"time"
)

const javaV1OwnerKey = "seata-java-v1-client-entry"

// 原Getty把EOF标记为不自动重连；显式入口在独立goroutine更换已断开的客户端。
// entry是会话创建时捕获的所有者，避免迟到回调关闭或复活另一个连接代际。
func (g *SessionManager) reconnectJavaV1(address string, entry *serverClientEntry) {
	if !g.isServerAddressAvailable(address) || !g.serverClients.CompareAndDelete(address, entry) {
		return
	}
	entry.Close()
	delay := time.Duration(g.gettyConf.ReconnectInterval)
	if delay <= 0 {
		delay = 300 * time.Millisecond
	}
	time.Sleep(delay)
	if !g.isServerAddressAvailable(address) {
		return
	}
	host, port, e := net.SplitHostPort(address)
	if e != nil {
		return
	}
	number, e := strconv.Atoi(port)
	if e != nil {
		return
	}
	g.ensureServerClient(&discovery.ServiceInstance{Addr: host, Port: number})
}

// sessionPackageHandler 为真实会话创建显式适配；选择在manager创建时捕获，重连保持相同选择。
func (g *SessionManager) sessionPackageHandler() (getty.ReadWriter, error) {
	if !g.javaV1Profile {
		return rpcPkgHandler, nil
	}
	if g.gettyConf.SessionConfig.CompressEncoding {
		return nil, fmt.Errorf("Java V1 profile requires Getty compression disabled")
	}
	profile, e := codec.NewJavaV1MessageProfile(codec.GetCodecManager())
	if e != nil {
		return nil, e
	}
	return &RpcPackageHandler{PayloadCodec: profile}, nil
}
