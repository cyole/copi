package client_test

import (
	"context"
	"net/http/httptest"
	"sync"
	"testing"
	"time"

	"github.com/cyole/copi/internal/client"
	"github.com/cyole/copi/internal/eventlog"
	"github.com/cyole/copi/internal/server"
)

type memoryClipboard struct {
	mu     sync.Mutex
	text   string
	writes int
}

func (m *memoryClipboard) ReadText() (string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.text, nil
}

func (m *memoryClipboard) WriteText(text string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.text = text
	m.writes++
	return nil
}

func (m *memoryClipboard) set(text string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.text = text
}

func (m *memoryClipboard) snapshot() (string, int) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.text, m.writes
}

func TestClientsSyncTextThroughRelay(t *testing.T) {
	relay := server.New(server.Options{
		Token:  "secret",
		Logger: eventlog.Discard(),
	})
	relayServer := httptest.NewServer(relay.Handler())
	defer relayServer.Close()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	clipA := &memoryClipboard{}
	clipB := &memoryClipboard{}
	errCh := make(chan error, 2)

	go func() {
		errCh <- client.Run(ctx, client.Options{
			ServerURL:    relayServer.URL,
			Token:        "secret",
			DeviceID:     "device-a",
			DeviceName:   "Device A",
			Interval:     10 * time.Millisecond,
			LongPollWait: 50 * time.Millisecond,
			Clipboard:    clipA,
			Logger:       eventlog.Discard(),
		})
	}()
	go func() {
		errCh <- client.Run(ctx, client.Options{
			ServerURL:    relayServer.URL,
			Token:        "secret",
			DeviceID:     "device-b",
			DeviceName:   "Device B",
			Interval:     10 * time.Millisecond,
			LongPollWait: 50 * time.Millisecond,
			Clipboard:    clipB,
			Logger:       eventlog.Discard(),
		})
	}()

	time.Sleep(30 * time.Millisecond)
	clipA.set("hello from A")

	deadline := time.After(2 * time.Second)
	tick := time.NewTicker(10 * time.Millisecond)
	defer tick.Stop()

	for {
		select {
		case <-deadline:
			text, writes := clipB.snapshot()
			t.Fatalf("timed out waiting for sync, B text=%q writes=%d", text, writes)
		case <-tick.C:
			text, writes := clipB.snapshot()
			if text == "hello from A" {
				if writes != 1 {
					t.Fatalf("B writes = %d, want 1", writes)
				}
				cancel()
				for i := 0; i < 2; i++ {
					if err := <-errCh; err != nil {
						t.Fatalf("client returned error: %v", err)
					}
				}
				return
			}
		}
	}
}
