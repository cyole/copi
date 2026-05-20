package client

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/cyole/copi/internal/clipboard"
	"github.com/cyole/copi/internal/eventlog"
	"github.com/cyole/copi/internal/protocol"
)

type Options struct {
	ServerURL    string
	Token        string
	DeviceID     string
	DeviceName   string
	Interval     time.Duration
	LongPollWait time.Duration
	Clipboard    clipboard.Provider
	Logger       *eventlog.Logger
}

func Run(ctx context.Context, opts Options) error {
	if opts.Clipboard == nil {
		return errors.New("clipboard provider is required")
	}
	if opts.Logger == nil {
		opts.Logger = eventlog.Discard()
	}
	if opts.Interval <= 0 {
		opts.Interval = 500 * time.Millisecond
	}
	if opts.LongPollWait <= 0 {
		opts.LongPollWait = 30 * time.Second
	}

	baseURL, err := normalizeURL(opts.ServerURL)
	if err != nil {
		return err
	}
	opts.Logger.Info("started", "client sync started", eventlog.Fields{
		"mode":        "client",
		"server":      baseURL,
		"device_id":   opts.DeviceID,
		"device_name": opts.DeviceName,
	})

	httpClient := &http.Client{Timeout: opts.LongPollWait + 10*time.Second}
	state := &syncState{}
	errCh := make(chan error, 2)

	go func() {
		errCh <- watchLocal(ctx, opts, baseURL, httpClient, state)
	}()
	go func() {
		errCh <- pullRemote(ctx, opts, baseURL, httpClient, state)
	}()

	select {
	case <-ctx.Done():
		return nil
	case err := <-errCh:
		if err == nil || errors.Is(err, context.Canceled) {
			return nil
		}
		return err
	}
}

type syncState struct {
	mu       sync.Mutex
	lastHash string
	sinceSeq uint64
}

func (s *syncState) SameHash(hash string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.lastHash == hash
}

func (s *syncState) MarkHash(hash string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.lastHash = hash
}

func (s *syncState) SinceSeq() uint64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.sinceSeq
}

func (s *syncState) MarkSeq(seq uint64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if seq > s.sinceSeq {
		s.sinceSeq = seq
	}
}

func watchLocal(ctx context.Context, opts Options, baseURL string, httpClient *http.Client, state *syncState) error {
	ticker := time.NewTicker(opts.Interval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return nil
		case <-ticker.C:
			text, err := opts.Clipboard.ReadText()
			if err != nil {
				opts.Logger.Error("clipboard_read_failed", "clipboard read failed", eventlog.Fields{
					"error": err.Error(),
				})
				continue
			}
			if text == "" {
				continue
			}

			payload := protocol.TextPayload(text)
			hash := protocol.HashPayload(payload)
			if state.SameHash(hash) {
				continue
			}

			env := protocol.NewEnvelope(opts.DeviceID, opts.DeviceName, payload)
			published, err := publish(ctx, httpClient, baseURL, opts.Token, env)
			if err != nil {
				opts.Logger.Error("publish_failed", "publish failed", eventlog.Fields{
					"error":  err.Error(),
					"server": baseURL,
				})
				continue
			}
			state.MarkHash(hash)
			state.MarkSeq(published.Seq)
			opts.Logger.Info("clipboard_published", "published clipboard text", eventlog.Fields{
				"bytes": len(text),
				"seq":   published.Seq,
			})
		}
	}
}

func pullRemote(ctx context.Context, opts Options, baseURL string, httpClient *http.Client, state *syncState) error {
	for {
		env, ok, err := poll(ctx, httpClient, baseURL, opts.Token, state.SinceSeq(), opts.LongPollWait)
		if err != nil {
			if errors.Is(err, context.Canceled) {
				return nil
			}
			opts.Logger.Error("poll_failed", "poll failed", eventlog.Fields{
				"error":  err.Error(),
				"server": baseURL,
			})
			sleep(ctx, 2*time.Second)
			continue
		}
		if !ok {
			continue
		}

		state.MarkSeq(env.Seq)
		if env.DeviceID == opts.DeviceID {
			continue
		}
		if env.Payload.Type != protocol.ContentTypeText {
			opts.Logger.Warn("unsupported_payload_ignored", "ignored unsupported payload type", eventlog.Fields{
				"payload_type": env.Payload.Type,
				"seq":          env.Seq,
			})
			continue
		}

		hash := env.Hash
		if hash == "" {
			hash = protocol.HashPayload(env.Payload)
		}
		if state.SameHash(hash) {
			continue
		}

		if err := opts.Clipboard.WriteText(env.Payload.Text); err != nil {
			opts.Logger.Error("clipboard_write_failed", "clipboard write failed", eventlog.Fields{
				"error": err.Error(),
				"seq":   env.Seq,
			})
			continue
		}
		state.MarkHash(hash)
		opts.Logger.Info("clipboard_applied", "applied clipboard text", eventlog.Fields{
			"bytes":            len(env.Payload.Text),
			"from_device_id":   env.DeviceID,
			"from_device_name": env.DeviceName,
			"seq":              env.Seq,
		})
	}
}

func publish(ctx context.Context, httpClient *http.Client, baseURL, token string, env protocol.Envelope) (protocol.Envelope, error) {
	body, err := json.Marshal(env)
	if err != nil {
		return protocol.Envelope{}, err
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, baseURL+"/v1/clipboard", bytes.NewReader(body))
	if err != nil {
		return protocol.Envelope{}, err
	}
	req.Header.Set("Content-Type", "application/json")
	setAuth(req, token)

	resp, err := httpClient.Do(req)
	if err != nil {
		return protocol.Envelope{}, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return protocol.Envelope{}, fmt.Errorf("server returned %s: %s", resp.Status, readSmall(resp.Body))
	}

	var published protocol.Envelope
	if err := json.NewDecoder(resp.Body).Decode(&published); err != nil {
		return protocol.Envelope{}, err
	}
	return published, nil
}

func poll(ctx context.Context, httpClient *http.Client, baseURL, token string, since uint64, wait time.Duration) (protocol.Envelope, bool, error) {
	u, err := url.Parse(baseURL + "/v1/clipboard")
	if err != nil {
		return protocol.Envelope{}, false, err
	}
	q := u.Query()
	q.Set("since", fmt.Sprintf("%d", since))
	q.Set("wait", wait.String())
	u.RawQuery = q.Encode()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u.String(), nil)
	if err != nil {
		return protocol.Envelope{}, false, err
	}
	setAuth(req, token)

	resp, err := httpClient.Do(req)
	if err != nil {
		return protocol.Envelope{}, false, err
	}
	defer resp.Body.Close()

	switch resp.StatusCode {
	case http.StatusOK:
		var env protocol.Envelope
		if err := json.NewDecoder(resp.Body).Decode(&env); err != nil {
			return protocol.Envelope{}, false, err
		}
		return env, true, nil
	case http.StatusNoContent:
		return protocol.Envelope{}, false, nil
	default:
		return protocol.Envelope{}, false, fmt.Errorf("server returned %s: %s", resp.Status, readSmall(resp.Body))
	}
}

func normalizeURL(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", errors.New("server URL is required")
	}
	if !strings.Contains(raw, "://") {
		raw = "http://" + raw
	}
	u, err := url.Parse(raw)
	if err != nil {
		return "", err
	}
	if u.Scheme != "http" && u.Scheme != "https" {
		return "", fmt.Errorf("unsupported server URL scheme: %s", u.Scheme)
	}
	if u.Host == "" {
		return "", errors.New("server URL must include a host")
	}
	return strings.TrimRight(u.String(), "/"), nil
}

func setAuth(req *http.Request, token string) {
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
}

func readSmall(r io.Reader) string {
	data, _ := io.ReadAll(io.LimitReader(r, 4096))
	return string(data)
}

func sleep(ctx context.Context, d time.Duration) {
	timer := time.NewTimer(d)
	defer timer.Stop()
	select {
	case <-ctx.Done():
	case <-timer.C:
	}
}
