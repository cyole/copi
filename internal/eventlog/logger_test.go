package eventlog

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestJSONLoggerWritesOneEventPerLine(t *testing.T) {
	var buf bytes.Buffer
	logger := New(&buf, FormatJSON)

	logger.Info("started", "client started", Fields{
		"mode": "client",
	})

	line := strings.TrimSpace(buf.String())
	if line == "" {
		t.Fatal("expected a JSON line")
	}

	var got map[string]any
	if err := json.Unmarshal([]byte(line), &got); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}

	if got["type"] != "started" {
		t.Fatalf("type = %v, want started", got["type"])
	}
	if got["level"] != "info" {
		t.Fatalf("level = %v, want info", got["level"])
	}
	if got["mode"] != "client" {
		t.Fatalf("mode = %v, want client", got["mode"])
	}
}

func TestParseFormat(t *testing.T) {
	for _, value := range []string{"", "text", "TEXT", "json", " JSON "} {
		if _, err := ParseFormat(value); err != nil {
			t.Fatalf("ParseFormat(%q) returned error: %v", value, err)
		}
	}

	if _, err := ParseFormat("xml"); err == nil {
		t.Fatal("expected unsupported format error")
	}
}
