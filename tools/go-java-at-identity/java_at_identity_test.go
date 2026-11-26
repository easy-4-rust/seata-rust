// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"strings"
	"testing"
)

func TestJavaATResourceIdentity(t *testing.T) {
	cases := []struct{ name, dsn, url, want string }{
		{"explicit", "user:secret@tcp(127.0.0.1:3306)/orders?parseTime=true", "jdbc:mysql://127.0.0.1:3306/orders?useSSL=false", "jdbc:mysql://127.0.0.1:3306/orders"},
		{"credential_rotation", "another:changed@tcp(127.0.0.1:3306)/orders", "jdbc:mysql://127.0.0.1:3306/orders", "jdbc:mysql://127.0.0.1:3306/orders"},
		{"implicit_jdbc_port", "user:secret@tcp(localhost:3306)/orders", "jdbc:mysql://localhost/orders", "jdbc:mysql://localhost/orders"},
		{"implicit_dsn_endpoint", "user:secret@/orders", "jdbc:mysql://127.0.0.1:3306/orders", "jdbc:mysql://127.0.0.1:3306/orders"},
		{"host_case_preserved", "user:secret@tcp(EXAMPLE.test:3306)/orders", "jdbc:mysql://example.TEST:3306/orders", "jdbc:mysql://example.TEST:3306/orders"},
		{"ipv6", "user:secret@tcp([::1]:3306)/orders", "jdbc:mysql://[::1]:3306/orders", "jdbc:mysql://[::1]:3306/orders"},
		{"loadbalance", "user:secret@tcp(second.test:3307)/orders", "jdbc:mysql:loadbalance://first.test:3306,second.test:3307/orders?x=y", "jdbc:mysql:loadbalance://first.test:3306|second.test:3307/orders"},
		{"query_credentials_removed", "user:secret@tcp(localhost:3306)/orders?charset=utf8mb4", "jdbc:mysql://localhost:3306/orders?user=java&password=hidden", "jdbc:mysql://localhost:3306/orders"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got, err := javaATResourceID(c.dsn, c.url)
			if err != nil || got != c.want {
				t.Fatalf("resource identity mismatch got=%q want=%q err=%v", got, c.want, err)
			}
			if strings.Contains(got, "secret") || strings.Contains(got, "password") {
				t.Fatal("resource leaked credentials")
			}
		})
	}
}
func TestJavaATResourceIdentityRejectsMismatch(t *testing.T) {
	cases := []struct{ name, dsn, url string }{
		{"empty_url", "user:secret@tcp(localhost:3306)/orders", ""},
		{"wrong_db", "user:secret@tcp(localhost:3306)/orders", "jdbc:mysql://localhost:3306/other"},
		{"wrong_port", "user:secret@tcp(localhost:3306)/orders", "jdbc:mysql://localhost:3307/orders"},
		{"alias_not_proven", "user:secret@tcp(127.0.0.1:3306)/orders", "jdbc:mysql://localhost:3306/orders"},
		{"url_userinfo", "user:secret@tcp(localhost:3306)/orders", "jdbc:mysql://java:hidden@localhost:3306/orders"},
		{"empty_db", "user:secret@tcp(localhost:3306)/", "jdbc:mysql://localhost:3306/"},
		{"unix_socket", "user:secret@unix(/tmp/mysql.sock)/orders", "jdbc:mysql://localhost:3306/orders"},
		{"unknown_scheme", "user:secret@tcp(localhost:3306)/orders", "jdbc:postgresql://localhost:3306/orders"},
		{"fragment", "user:secret@tcp(localhost:3306)/orders", "jdbc:mysql://localhost:3306/orders#hidden"},
		{"loadbalance_endpoint_absent", "user:secret@tcp(third.test:3306)/orders", "jdbc:mysql:loadbalance://first.test:3306,second.test:3306/orders"},
		{"bad_dsn", "user:secret@broken", "jdbc:mysql://localhost:3306/orders"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			_, err := javaATResourceID(c.dsn, c.url)
			if err == nil {
				t.Fatal("unsafe or mismatched identity accepted")
			}
			if strings.Contains(err.Error(), "secret") || strings.Contains(err.Error(), "hidden") {
				t.Fatal("identity error leaked credential")
			}
		})
	}
}
func TestJavaATLegacyResourceUnchanged(t *testing.T) {
	if got := parseResourceID("user:secret@tcp(localhost:3306)/orders?parseTime=true"); got != "user:secret@tcp(localhost:3306)/orders" {
		t.Fatal("legacy resource changed")
	}
}
