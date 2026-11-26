// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.
package codec_test

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"

	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	seataerrors "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type javaLengthRow struct {
	Name       string  `json:"name"`
	TypeID     int     `json:"type_id"`
	ResultCode int     `json:"result_code"`
	ErrorCode  int     `json:"error_code"`
	Message    *string `json:"message"`
	Lockable   bool    `json:"lockable"`
	Hex        string  `json:"serialized_hex"`
}

// TestJavaV1ErrorLength catches an 8-bit length prefix, text truncation at127,
// or a decoder reading the error/lockable fields at the wrong byte offset.
func TestJavaV1ErrorLength(t *testing.T) {
	raw, err := os.ReadFile(os.Getenv("SEATA_JAVA_LENGTH_VECTORS"))
	if err != nil {
		t.Fatal(err)
	}
	var rows []javaLengthRow
	if err := json.Unmarshal(raw, &rows); err != nil {
		t.Fatal(err)
	}
	if len(rows) != 24 {
		t.Fatalf("need24 independent Java cases, got%d", len(rows))
	}
	inputs := map[string]string{
		"Empty": "", "AsciiOne": "x", "Ascii127": strings.Repeat("x", 127),
		"Ascii128": strings.Repeat("x", 128), "Ascii255": strings.Repeat("x", 255),
		"Ascii256": strings.Repeat("x", 256), "Ascii32767": strings.Repeat("x", 32767),
		"Ascii32768": strings.Repeat("x", 32768), "Utf8Short": strings.Repeat("报错", 22),
		"Utf8Boundary": strings.Repeat("错", 10922),
	}
	codec.Init()
	manager := codec.GetCodecManager()
	seen := make(map[string]bool)
	for _, row := range rows {
		row := row
		if seen[row.Name] {
			t.Fatalf("duplicate case%s", row.Name)
		}
		seen[row.Name] = true
		t.Run(row.Name, func(t *testing.T) {
			family, suffix, ok := strings.Cut(row.Name, "_")
			if !ok || (family != "Branch" && family != "Lock") {
				t.Fatalf("unexpected case%s", row.Name)
			}
			text, known := inputs[suffix]
			success := suffix == "Success" || suffix == "SuccessFalse" || suffix == "SuccessTrue"
			if suffix == "FailedTrue" {
				text = "报错"
				known = true
			}
			if !known && !success {
				t.Fatalf("unexpected case%s", row.Name)
			}
			lockable := suffix == "SuccessTrue" || suffix == "FailedTrue"
			result := message.ResultCodeFailed
			code := seataerrors.TransactionErrorCodeBranchReportFailed
			typeID := 14
			if family == "Lock" {
				code = seataerrors.TransactionErrorCodeLockableCheckFailed
				typeID = 22
			}
			if success {
				result = message.ResultCodeSuccess
				code = seataerrors.TransactionErrorCodeUnknown
			}
			base := message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: result, Msg: text}, TransactionErrorCode: code}
			var value message.MessageTypeAware = message.BranchReportResponse{AbstractTransactionResponse: base}
			if family == "Lock" {
				value = message.GlobalLockQueryResponse{AbstractTransactionResponse: base, Lockable: lockable}
			}
			decodedText := text
			if len(decodedText) > 32767 {
				decodedText = decodedText[:32767]
			}
			if success {
				decodedText = ""
			}
			if row.TypeID != typeID || row.ResultCode != int(result) || row.ErrorCode != int(code) || row.Lockable != lockable {
				t.Fatal("Java semantic metadata differs from independent case")
			}
			if row.Message == nil {
				if decodedText != "" {
					t.Fatal("Java missing expected message")
				}
			} else if *row.Message != decodedText {
				t.Fatal("Java decoded text differs")
			}
			wire, err := hex.DecodeString(row.Hex)
			if err != nil {
				t.Fatal(err)
			}
			encoded := manager.Encode(codec.CodecTypeSeata, value)
			if !bytes.Equal(encoded, wire) {
				t.Errorf("Java encode mismatch: Go bytes=%d, Java bytes=%d", len(encoded), len(wire))
			}
			base.Msg = decodedText
			var expected message.MessageTypeAware = message.BranchReportResponse{AbstractTransactionResponse: base}
			if family == "Lock" {
				expected = message.GlobalLockQueryResponse{AbstractTransactionResponse: base, Lockable: lockable}
			}
			decoded := manager.Decode(codec.CodecTypeSeata, wire)
			if !reflect.DeepEqual(decoded, expected) {
				t.Errorf("Java decode lost text/error/lockable fields (expected message bytes=%d)", len(decodedText))
				return
			}
			if !bytes.Equal(manager.Encode(codec.CodecTypeSeata, decoded), wire) {
				t.Error("Java input not byte-stable on Go re-encode")
			}
		})
	}
	for suffix := range inputs {
		for _, family := range []string{"Branch", "Lock"} {
			if !seen[family+"_"+suffix] {
				t.Fatalf("missing%s_%s", family, suffix)
			}
		}
	}
	for _, name := range []string{"Branch_Success", "Lock_SuccessFalse", "Lock_SuccessTrue", "Lock_FailedTrue"} {
		if !seen[name] {
			t.Fatalf("missing%s", name)
		}
	}
}
