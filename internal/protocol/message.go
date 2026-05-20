package protocol

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"time"
)

const MaxPayloadBytes int64 = 10 * 1024 * 1024

type ContentType string

const (
	ContentTypeText ContentType = "text"
)

type Payload struct {
	Type ContentType `json:"type"`
	Text string      `json:"text,omitempty"`
	MIME string      `json:"mime,omitempty"`
	Data string      `json:"data,omitempty"`
}

type Envelope struct {
	ID         string    `json:"id"`
	DeviceID   string    `json:"device_id"`
	DeviceName string    `json:"device_name,omitempty"`
	Seq        uint64    `json:"seq,omitempty"`
	Timestamp  time.Time `json:"timestamp"`
	Payload    Payload   `json:"payload"`
	Hash       string    `json:"hash"`
}

func TextPayload(text string) Payload {
	return Payload{
		Type: ContentTypeText,
		Text: text,
		MIME: "text/plain; charset=utf-8",
	}
}

func NewEnvelope(deviceID, deviceName string, payload Payload) Envelope {
	now := time.Now().UTC()
	return Envelope{
		ID:         fmt.Sprintf("%s-%d", deviceID, now.UnixNano()),
		DeviceID:   deviceID,
		DeviceName: deviceName,
		Timestamp:  now,
		Payload:    payload,
		Hash:       HashPayload(payload),
	}
}

func HashPayload(payload Payload) string {
	h := sha256.New()
	_, _ = h.Write([]byte(payload.Type))
	_, _ = h.Write([]byte{0})
	_, _ = h.Write([]byte(payload.MIME))
	_, _ = h.Write([]byte{0})
	_, _ = h.Write([]byte(payload.Text))
	_, _ = h.Write([]byte{0})
	_, _ = h.Write([]byte(payload.Data))
	return hex.EncodeToString(h.Sum(nil))
}
