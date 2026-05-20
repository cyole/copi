package doctor

import (
	"context"
	"testing"
	"time"

	"github.com/cyole/copi/internal/config"
)

type fakeClipboard struct {
	text string
	err  error
}

func (f fakeClipboard) ReadText() (string, error) {
	return f.text, f.err
}

func (f fakeClipboard) WriteText(string) error {
	return nil
}

func TestRunDoctorReportsWarningsWithoutFailing(t *testing.T) {
	cfg := config.Default()
	cfg.Token = ""
	cfg.LAN.ListenAddr = "127.0.0.1:0"

	report := Run(context.Background(), Options{
		ConfigPath:     "/tmp/does-not-exist-copi.json",
		Config:         cfg,
		Mode:           "all",
		Timeout:        time.Millisecond,
		CheckClipboard: true,
		Clipboard:      fakeClipboard{text: "hello"},
	})

	if !report.OK {
		t.Fatalf("report should be OK with warnings: %#v", report)
	}
	if report.Summary.Warn == 0 {
		t.Fatal("expected at least one warning")
	}
	if report.Summary.Fail != 0 {
		t.Fatalf("fail count = %d, want 0", report.Summary.Fail)
	}
}

func TestRunDoctorFailsInvalidDuration(t *testing.T) {
	cfg := config.Default()
	cfg.Client.Interval = "not-a-duration"
	cfg.LAN.ListenAddr = "127.0.0.1:0"

	report := Run(context.Background(), Options{
		ConfigPath: "/tmp/does-not-exist-copi.json",
		Config:     cfg,
		Mode:       "all",
		Timeout:    time.Millisecond,
	})

	if report.OK {
		t.Fatal("expected doctor failure")
	}
	if report.Summary.Fail == 0 {
		t.Fatal("expected failing checks")
	}
}

func TestRunDoctorFailsInvalidMode(t *testing.T) {
	report := Run(context.Background(), Options{
		ConfigPath: "/tmp/does-not-exist-copi.json",
		Config:     config.Default(),
		Mode:       "weird",
		Timeout:    time.Millisecond,
	})

	if report.OK {
		t.Fatal("expected doctor failure")
	}
	if report.Summary.Fail != 1 {
		t.Fatalf("fail count = %d, want 1", report.Summary.Fail)
	}
}
