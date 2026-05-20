package protocol

import "testing"

func TestHashPayloadChangesWithContent(t *testing.T) {
	first := HashPayload(TextPayload("hello"))
	second := HashPayload(TextPayload("hello"))
	third := HashPayload(TextPayload("goodbye"))

	if first == "" {
		t.Fatal("expected non-empty hash")
	}
	if first != second {
		t.Fatal("same payload should produce the same hash")
	}
	if first == third {
		t.Fatal("different payload should produce a different hash")
	}
}
