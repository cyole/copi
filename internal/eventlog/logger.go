package eventlog

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"sort"
	"strings"
	"sync"
	"time"
)

type Format string

const (
	FormatText Format = "text"
	FormatJSON Format = "json"
)

type Fields map[string]any

type Logger struct {
	mu     sync.Mutex
	out    io.Writer
	format Format
	text   *log.Logger
	now    func() time.Time
}

func ParseFormat(raw string) (Format, error) {
	switch Format(strings.ToLower(strings.TrimSpace(raw))) {
	case "", FormatText:
		return FormatText, nil
	case FormatJSON:
		return FormatJSON, nil
	default:
		return "", fmt.Errorf("unsupported log format %q, expected text or json", raw)
	}
}

func New(out io.Writer, format Format) *Logger {
	if out == nil {
		out = io.Discard
	}
	return &Logger{
		out:    out,
		format: format,
		text:   log.New(out, "", log.LstdFlags),
		now:    func() time.Time { return time.Now().UTC() },
	}
}

func Discard() *Logger {
	return New(io.Discard, FormatText)
}

func (l *Logger) Printf(format string, args ...any) {
	if l == nil {
		return
	}
	l.Event("info", "log", fmt.Sprintf(format, args...), nil)
}

func (l *Logger) Info(eventType, message string, fields Fields) {
	l.Event("info", eventType, message, fields)
}

func (l *Logger) Warn(eventType, message string, fields Fields) {
	l.Event("warn", eventType, message, fields)
}

func (l *Logger) Error(eventType, message string, fields Fields) {
	l.Event("error", eventType, message, fields)
}

func (l *Logger) Event(level, eventType, message string, fields Fields) {
	if l == nil {
		return
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	if l.format != FormatJSON {
		if message == "" {
			message = eventType
		}
		if len(fields) == 0 {
			l.text.Printf("%s", message)
			return
		}
		l.text.Printf("%s %s", message, formatFields(fields))
		return
	}

	payload := map[string]any{
		"time":  l.now().Format(time.RFC3339Nano),
		"level": level,
		"type":  eventType,
	}
	if message != "" {
		payload["message"] = message
	}
	for key, value := range fields {
		if isReservedKey(key) {
			continue
		}
		payload[key] = value
	}
	_ = json.NewEncoder(l.out).Encode(payload)
}

func formatFields(fields Fields) string {
	keys := make([]string, 0, len(fields))
	for key := range fields {
		keys = append(keys, key)
	}
	sort.Strings(keys)

	parts := make([]string, 0, len(keys))
	for _, key := range keys {
		parts = append(parts, fmt.Sprintf("%s=%v", key, fields[key]))
	}
	return strings.Join(parts, " ")
}

func isReservedKey(key string) bool {
	switch key {
	case "time", "level", "type", "message":
		return true
	default:
		return false
	}
}
