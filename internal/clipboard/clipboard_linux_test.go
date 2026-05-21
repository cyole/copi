//go:build linux

package clipboard

import (
	"testing"
)

func TestPreferredLinuxCommandsUseUTF8Targets(t *testing.T) {
	t.Setenv("WAYLAND_DISPLAY", "wayland-test")

	read := preferredLinuxReadCommands()
	write := preferredLinuxWriteCommands()

	if len(read) == 0 || !commandContains(read[0], utf8TextMIME) {
		t.Fatalf("first Wayland read command = %#v, want UTF-8 MIME", read)
	}
	if len(write) == 0 || !commandContains(write[0], utf8TextMIME) {
		t.Fatalf("first Wayland write command = %#v, want UTF-8 MIME", write)
	}
}

func TestPreferredLinuxCommandsPreferX11WithoutWayland(t *testing.T) {
	t.Setenv("WAYLAND_DISPLAY", "")

	read := preferredLinuxReadCommands()
	write := preferredLinuxWriteCommands()

	if len(read) == 0 || read[0][0] != "xclip" || !commandContains(read[0], "UTF8_STRING") {
		t.Fatalf("first X11 read command = %#v, want xclip UTF8_STRING", read)
	}
	if len(write) == 0 || write[0][0] != "xclip" || !commandContains(write[0], "UTF8_STRING") {
		t.Fatalf("first X11 write command = %#v, want xclip UTF8_STRING", write)
	}
}

func commandContains(command []string, value string) bool {
	for _, arg := range command {
		if arg == value {
			return true
		}
	}
	return false
}
