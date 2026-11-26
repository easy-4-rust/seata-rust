// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main
import("fmt";"os";"seata.apache.org/seata-go/v2/pkg/client")
func main(){if len(os.Args)!=2{panic("one owned configuration path required")};client.InitPath(os.Args[1]);fmt.Println("FULL_CLIENT_INIT_RETURNED")}
