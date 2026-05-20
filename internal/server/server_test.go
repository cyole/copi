package server

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/cyole/copi/internal/protocol"
)

func TestPublishAndPollLatestClipboard(t *testing.T) {
	srv := New(Options{Token: "secret"})
	testServer := httptest.NewServer(srv.routes())
	defer testServer.Close()

	env := protocol.NewEnvelope("device-a", "Device A", protocol.TextPayload("hello"))
	body, err := json.Marshal(env)
	if err != nil {
		t.Fatal(err)
	}

	req, err := http.NewRequest(http.MethodPost, testServer.URL+"/v1/clipboard", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Authorization", "Bearer secret")
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("publish status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var published protocol.Envelope
	if err := json.NewDecoder(resp.Body).Decode(&published); err != nil {
		t.Fatal(err)
	}
	if published.Seq != 1 {
		t.Fatalf("seq = %d, want 1", published.Seq)
	}

	req, err = http.NewRequest(http.MethodGet, testServer.URL+"/v1/clipboard?since=0&wait=1ms", nil)
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Authorization", "Bearer secret")

	resp, err = http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("poll status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var got protocol.Envelope
	if err := json.NewDecoder(resp.Body).Decode(&got); err != nil {
		t.Fatal(err)
	}
	if got.Payload.Text != "hello" {
		t.Fatalf("payload text = %q, want hello", got.Payload.Text)
	}
}

func TestTokenRequired(t *testing.T) {
	srv := New(Options{Token: "secret"})
	testServer := httptest.NewServer(srv.routes())
	defer testServer.Close()

	resp, err := http.Post(testServer.URL+"/v1/clipboard", "application/json", bytes.NewReader([]byte("{}")))
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("status = %d, want %d", resp.StatusCode, http.StatusUnauthorized)
	}
}
