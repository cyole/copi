package server

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/cyole/copi/internal/protocol"
)

type Options struct {
	Addr   string
	Token  string
	Logger *log.Logger
}

type Server struct {
	opts Options
	hub  *Hub
}

type Hub struct {
	mu          sync.Mutex
	seq         uint64
	latest      *protocol.Envelope
	subscribers map[chan protocol.Envelope]struct{}
}

func New(opts Options) *Server {
	if opts.Logger == nil {
		opts.Logger = log.Default()
	}
	return &Server{
		opts: opts,
		hub: &Hub{
			subscribers: make(map[chan protocol.Envelope]struct{}),
		},
	}
}

func (s *Server) Run(ctx context.Context) error {
	httpServer := &http.Server{
		Addr:    s.opts.Addr,
		Handler: s.routes(),
	}

	listener, err := net.Listen("tcp", s.opts.Addr)
	if err != nil {
		return err
	}

	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = httpServer.Shutdown(shutdownCtx)
	}()

	s.opts.Logger.Printf("HTTP relay listening on http://%s", listener.Addr().String())
	err = httpServer.Serve(listener)
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func (s *Server) routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok\n"))
	})
	mux.HandleFunc("/v1/clipboard", s.handleClipboard)
	return mux
}

func (s *Server) handleClipboard(w http.ResponseWriter, r *http.Request) {
	if !Authorized(r, s.opts.Token) {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	switch r.Method {
	case http.MethodPost:
		s.handlePublish(w, r)
	case http.MethodGet:
		s.handlePoll(w, r)
	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func (s *Server) handlePublish(w http.ResponseWriter, r *http.Request) {
	var env protocol.Envelope
	if err := json.NewDecoder(io.LimitReader(r.Body, protocol.MaxPayloadBytes)).Decode(&env); err != nil {
		http.Error(w, "invalid json", http.StatusBadRequest)
		return
	}
	if err := validateEnvelope(env); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	env = s.hub.Publish(env)
	s.opts.Logger.Printf("published seq=%d from %s (%s)", env.Seq, env.DeviceName, env.DeviceID)
	writeJSON(w, http.StatusOK, env)
}

func (s *Server) handlePoll(w http.ResponseWriter, r *http.Request) {
	since, err := parseUintQuery(r, "since", 0)
	if err != nil {
		http.Error(w, "invalid since", http.StatusBadRequest)
		return
	}
	wait := 30 * time.Second
	if raw := r.URL.Query().Get("wait"); raw != "" {
		parsed, err := time.ParseDuration(raw)
		if err != nil {
			http.Error(w, "invalid wait", http.StatusBadRequest)
			return
		}
		if parsed > 0 && parsed <= 2*time.Minute {
			wait = parsed
		}
	}

	env, ok, err := s.hub.WaitSince(r.Context(), since, wait)
	if err != nil {
		return
	}
	if !ok {
		w.WriteHeader(http.StatusNoContent)
		return
	}
	writeJSON(w, http.StatusOK, env)
}

func (h *Hub) Publish(env protocol.Envelope) protocol.Envelope {
	h.mu.Lock()
	defer h.mu.Unlock()

	h.seq++
	env.Seq = h.seq
	if env.Timestamp.IsZero() {
		env.Timestamp = time.Now().UTC()
	}
	if env.Hash == "" {
		env.Hash = protocol.HashPayload(env.Payload)
	}
	h.latest = &env

	for ch := range h.subscribers {
		select {
		case ch <- env:
		default:
		}
	}
	return env
}

func (h *Hub) WaitSince(ctx context.Context, since uint64, wait time.Duration) (protocol.Envelope, bool, error) {
	h.mu.Lock()
	if h.latest != nil && h.latest.Seq > since {
		env := *h.latest
		h.mu.Unlock()
		return env, true, nil
	}

	ch := make(chan protocol.Envelope, 1)
	h.subscribers[ch] = struct{}{}
	h.mu.Unlock()

	defer func() {
		h.mu.Lock()
		delete(h.subscribers, ch)
		h.mu.Unlock()
	}()

	timer := time.NewTimer(wait)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		return protocol.Envelope{}, false, ctx.Err()
	case <-timer.C:
		return protocol.Envelope{}, false, nil
	case env := <-ch:
		if env.Seq <= since {
			return protocol.Envelope{}, false, nil
		}
		return env, true, nil
	}
}

func Authorized(r *http.Request, token string) bool {
	if token == "" {
		return true
	}
	auth := r.Header.Get("Authorization")
	if strings.HasPrefix(auth, "Bearer ") && strings.TrimPrefix(auth, "Bearer ") == token {
		return true
	}
	return r.URL.Query().Get("token") == token
}

func validateEnvelope(env protocol.Envelope) error {
	if strings.TrimSpace(env.DeviceID) == "" {
		return errors.New("device_id is required")
	}
	switch env.Payload.Type {
	case protocol.ContentTypeText:
		if env.Payload.Text == "" {
			return errors.New("text payload is empty")
		}
	default:
		return fmt.Errorf("unsupported payload type: %s", env.Payload.Type)
	}
	return nil
}

func parseUintQuery(r *http.Request, key string, fallback uint64) (uint64, error) {
	raw := r.URL.Query().Get(key)
	if raw == "" {
		return fallback, nil
	}
	return strconv.ParseUint(raw, 10, 64)
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
