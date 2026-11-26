// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty_test

import (
	"flag"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	"testing"
)

func TestJavaV1SDKProfileFlagDefaultAndExplicit(t *testing.T) {
	var cfg config.Config
	flags := flag.NewFlagSet("profile", flag.ContinueOnError)
	cfg.RegisterFlagsWithPrefix("getty", flags)
	if e := flags.Parse([]string{"-getty.java-v1-profile=true"}); e != nil {
		t.Fatal("explicit SDK selection is unavailable", e)
	}
	field := reflect.ValueOf(cfg).FieldByName("JavaV1Profile")
	if !field.IsValid() || !field.Bool() {
		t.Fatal("explicit profile not enabled")
	}
	var defaults config.Config
	flagSet := flag.NewFlagSet("defaults", flag.ContinueOnError)
	defaults.RegisterFlagsWithPrefix("getty", flagSet)
	if reflect.ValueOf(defaults).FieldByName("JavaV1Profile").Bool() {
		t.Fatal("default enabled compatibility globally")
	}
}
