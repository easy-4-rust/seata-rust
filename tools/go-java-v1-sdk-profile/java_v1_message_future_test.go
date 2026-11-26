// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package message_test

import (
	"errors"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"sync"
	"sync/atomic"
	"testing"
)

func TestJavaV1FuturePublishesOneCoherentSuccessOrError(t *testing.T) {
	cause := errors.New("unknown outcome")
	for round := 0; round < 64; round++ {
		f := message.NewMessageFuture(message.RpcMessage{ID: int32(round)})
		start := make(chan struct{})
		var wait sync.WaitGroup
		var wins atomic.Int32
		for i := 0; i < 32; i++ {
			wait.Add(1)
			go func(index int) {
				defer wait.Done()
				<-start
				var won bool
				if index%2 == 0 {
					won = f.Complete("ok")
				} else {
					won = f.CompleteWithError(nil, cause)
				}
				if won {
					wins.Add(1)
				}
			}(i)
		}
		close(start)
		wait.Wait()
		if wins.Load() != 1 {
			t.Fatal("multiple owners")
		}
		<-f.Done
		if (f.Response == nil && !errors.Is(f.Err, cause)) || (f.Response != nil && (f.Response != "ok" || f.Err != nil)) {
			t.Fatal("response/error came from different owners")
		}
		select {
		case <-f.Done:
			t.Fatal("duplicate signal")
		default:
		}
	}
}
