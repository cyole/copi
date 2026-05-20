package lan

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/cyole/copi/internal/clipboard"
	"github.com/cyole/copi/internal/eventlog"
	"github.com/cyole/copi/internal/protocol"
	"github.com/cyole/copi/internal/server"
)

const DefaultMulticastAddress = "239.255.27.42:9529"

type Options struct {
	ListenAddr    string
	AdvertiseURL  string
	MulticastAddr string
	Token         string
	DeviceID      string
	DeviceName    string
	Interval      time.Duration
	Clipboard     clipboard.Provider
	Logger        *eventlog.Logger
}

type announcement struct {
	DeviceID   string    `json:"device_id"`
	DeviceName string    `json:"device_name"`
	URL        string    `json:"url"`
	Time       time.Time `json:"time"`
}

type peer struct {
	DeviceID   string
	DeviceName string
	URL        string
	LastSeen   time.Time
}

type peerStore struct {
	mu    sync.Mutex
	peers map[string]peer
}

type syncState struct {
	mu       sync.Mutex
	lastHash string
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
	if opts.MulticastAddr == "" {
		opts.MulticastAddr = DefaultMulticastAddress
	}

	advertiseURL := strings.TrimRight(opts.AdvertiseURL, "/")
	if advertiseURL == "" {
		var err error
		advertiseURL, err = inferAdvertiseURL(opts.ListenAddr)
		if err != nil {
			return err
		}
	}
	opts.Logger.Info("started", "LAN sync started", eventlog.Fields{
		"mode":           "lan",
		"listen":         opts.ListenAddr,
		"advertise_url":  advertiseURL,
		"multicast_addr": opts.MulticastAddr,
		"device_id":      opts.DeviceID,
		"device_name":    opts.DeviceName,
	})

	store := &peerStore{peers: make(map[string]peer)}
	state := &syncState{}
	httpClient := &http.Client{Timeout: 5 * time.Second}
	errCh := make(chan error, 5)

	srv := &http.Server{
		Addr:    opts.ListenAddr,
		Handler: lanHandler(opts, state),
	}

	go func() {
		opts.Logger.Info("lan_peer_listening", "LAN peer HTTP listening", eventlog.Fields{
			"listen":        opts.ListenAddr,
			"advertise_url": advertiseURL,
		})
		err := srv.ListenAndServe()
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			errCh <- err
			return
		}
		errCh <- nil
	}()
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdownCtx)
	}()
	go func() {
		errCh <- announceLoop(ctx, opts, advertiseURL)
	}()
	go func() {
		errCh <- listenLoop(ctx, opts, store)
	}()
	go func() {
		errCh <- watchAndBroadcast(ctx, opts, httpClient, store, state)
	}()
	go func() {
		errCh <- cleanupLoop(ctx, store)
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

func lanHandler(opts Options, state *syncState) http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok\n"))
	})
	mux.HandleFunc("/v1/clipboard", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		if !server.Authorized(r, opts.Token) {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}

		var env protocol.Envelope
		if err := json.NewDecoder(io.LimitReader(r.Body, protocol.MaxPayloadBytes)).Decode(&env); err != nil {
			http.Error(w, "invalid json", http.StatusBadRequest)
			return
		}
		if env.DeviceID == opts.DeviceID {
			writeJSON(w, env)
			return
		}
		if env.Payload.Type != protocol.ContentTypeText {
			http.Error(w, "unsupported payload type", http.StatusBadRequest)
			return
		}

		hash := env.Hash
		if hash == "" {
			hash = protocol.HashPayload(env.Payload)
		}
		if state.same(hash) {
			writeJSON(w, env)
			return
		}
		if err := opts.Clipboard.WriteText(env.Payload.Text); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		state.mark(hash)
		opts.Logger.Info("clipboard_applied", "applied LAN clipboard text", eventlog.Fields{
			"bytes":            len(env.Payload.Text),
			"from_device_id":   env.DeviceID,
			"from_device_name": env.DeviceName,
		})
		writeJSON(w, env)
	})
	return mux
}

func watchAndBroadcast(ctx context.Context, opts Options, httpClient *http.Client, store *peerStore, state *syncState) error {
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
			if state.same(hash) {
				continue
			}
			state.mark(hash)
			env := protocol.NewEnvelope(opts.DeviceID, opts.DeviceName, payload)
			peers := store.list()
			for _, p := range peers {
				if err := postPeer(ctx, httpClient, opts.Token, p.URL, env); err != nil {
					opts.Logger.Error("peer_sync_failed", "sync to peer failed", eventlog.Fields{
						"error":            err.Error(),
						"peer_device_id":   p.DeviceID,
						"peer_device_name": p.DeviceName,
						"peer_url":         p.URL,
					})
				}
			}
			if len(peers) > 0 {
				opts.Logger.Info("clipboard_broadcast", "broadcast clipboard text to LAN peers", eventlog.Fields{
					"bytes":      len(text),
					"peer_count": len(peers),
				})
			}
		}
	}
}

func announceLoop(ctx context.Context, opts Options, advertiseURL string) error {
	addr, err := net.ResolveUDPAddr("udp4", opts.MulticastAddr)
	if err != nil {
		return err
	}
	conn, err := net.DialUDP("udp4", nil, addr)
	if err != nil {
		return err
	}
	defer conn.Close()

	ticker := time.NewTicker(3 * time.Second)
	defer ticker.Stop()

	send := func() {
		payload, _ := json.Marshal(announcement{
			DeviceID:   opts.DeviceID,
			DeviceName: opts.DeviceName,
			URL:        advertiseURL,
			Time:       time.Now().UTC(),
		})
		_, _ = conn.Write(payload)
	}

	send()
	for {
		select {
		case <-ctx.Done():
			return nil
		case <-ticker.C:
			send()
		}
	}
}

func listenLoop(ctx context.Context, opts Options, store *peerStore) error {
	addr, err := net.ResolveUDPAddr("udp4", opts.MulticastAddr)
	if err != nil {
		return err
	}
	conn, err := net.ListenMulticastUDP("udp4", nil, addr)
	if err != nil {
		return err
	}
	defer conn.Close()
	_ = conn.SetReadBuffer(64 * 1024)
	_ = conn.SetReadDeadline(time.Now().Add(time.Second))

	buf := make([]byte, 4096)
	for {
		select {
		case <-ctx.Done():
			return nil
		default:
		}

		n, _, err := conn.ReadFromUDP(buf)
		if err != nil {
			if ne, ok := err.(net.Error); ok && ne.Timeout() {
				_ = conn.SetReadDeadline(time.Now().Add(time.Second))
				continue
			}
			return err
		}
		_ = conn.SetReadDeadline(time.Now().Add(time.Second))

		var ann announcement
		if err := json.Unmarshal(buf[:n], &ann); err != nil {
			continue
		}
		if ann.DeviceID == "" || ann.DeviceID == opts.DeviceID || ann.URL == "" {
			continue
		}
		p := peer{
			DeviceID:   ann.DeviceID,
			DeviceName: ann.DeviceName,
			URL:        strings.TrimRight(ann.URL, "/"),
			LastSeen:   time.Now(),
		}
		if store.upsert(p) {
			opts.Logger.Info("peer_discovered", "LAN peer discovered", eventlog.Fields{
				"peer_device_id":   p.DeviceID,
				"peer_device_name": p.DeviceName,
				"peer_url":         p.URL,
			})
		}
	}
}

func cleanupLoop(ctx context.Context, store *peerStore) error {
	ticker := time.NewTicker(10 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return nil
		case <-ticker.C:
			store.cleanup(20 * time.Second)
		}
	}
}

func postPeer(ctx context.Context, httpClient *http.Client, token, peerURL string, env protocol.Envelope) error {
	body, err := json.Marshal(env)
	if err != nil {
		return err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, strings.TrimRight(peerURL, "/")+"/v1/clipboard", bytes.NewReader(body))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}

	resp, err := httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return fmt.Errorf("%s: %s", resp.Status, string(data))
	}
	return nil
}

func inferAdvertiseURL(listenAddr string) (string, error) {
	host, port, err := splitListenAddr(listenAddr)
	if err != nil {
		return "", err
	}
	if host == "" || host == "0.0.0.0" || host == "::" || host == "[::]" {
		host = outboundIP()
	}
	return "http://" + net.JoinHostPort(host, port), nil
}

func splitListenAddr(addr string) (string, string, error) {
	host, port, err := net.SplitHostPort(addr)
	if err == nil {
		return strings.Trim(host, "[]"), port, nil
	}
	if strings.HasPrefix(addr, ":") {
		return "", strings.TrimPrefix(addr, ":"), nil
	}
	if p, err := strconv.Atoi(addr); err == nil && p > 0 {
		return "", addr, nil
	}
	return "", "", err
}

func outboundIP() string {
	conn, err := net.Dial("udp4", "8.8.8.8:80")
	if err == nil {
		defer conn.Close()
		if local, ok := conn.LocalAddr().(*net.UDPAddr); ok && local.IP != nil {
			return local.IP.String()
		}
	}

	ifaces, err := net.Interfaces()
	if err != nil {
		return "127.0.0.1"
	}
	for _, iface := range ifaces {
		if iface.Flags&net.FlagUp == 0 || iface.Flags&net.FlagLoopback != 0 {
			continue
		}
		addrs, err := iface.Addrs()
		if err != nil {
			continue
		}
		for _, addr := range addrs {
			ip, _, err := net.ParseCIDR(addr.String())
			if err != nil || ip == nil {
				continue
			}
			ip = ip.To4()
			if ip != nil && !ip.IsLoopback() {
				return ip.String()
			}
		}
	}
	return "127.0.0.1"
}

func (s *peerStore) upsert(p peer) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, existed := s.peers[p.DeviceID]
	s.peers[p.DeviceID] = p
	return !existed
}

func (s *peerStore) list() []peer {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]peer, 0, len(s.peers))
	for _, p := range s.peers {
		out = append(out, p)
	}
	return out
}

func (s *peerStore) cleanup(maxAge time.Duration) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cutoff := time.Now().Add(-maxAge)
	for id, p := range s.peers {
		if p.LastSeen.Before(cutoff) {
			delete(s.peers, id)
		}
	}
}

func (s *syncState) same(hash string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.lastHash == hash
}

func (s *syncState) mark(hash string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.lastHash = hash
}

func writeJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(value)
}
