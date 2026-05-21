package clipboard

import "testing"

func TestWithEnvOverridesReplacesExistingValues(t *testing.T) {
	env := withEnvOverrides([]string{
		"PATH=/bin",
		"LANG=C",
		"LC_CTYPE=C",
	}, map[string]string{
		"LANG":     "C.UTF-8",
		"LC_CTYPE": "C.UTF-8",
	})

	got := map[string]string{}
	for _, entry := range env {
		key, value, ok := cutEnv(entry)
		if ok {
			got[key] = value
		}
	}

	if got["PATH"] != "/bin" {
		t.Fatalf("PATH = %q, want /bin", got["PATH"])
	}
	if got["LANG"] != "C.UTF-8" {
		t.Fatalf("LANG = %q, want C.UTF-8", got["LANG"])
	}
	if got["LC_CTYPE"] != "C.UTF-8" {
		t.Fatalf("LC_CTYPE = %q, want C.UTF-8", got["LC_CTYPE"])
	}
}

func TestValidUTF8StringReplacesInvalidBytes(t *testing.T) {
	got := validUTF8String([]byte{'o', 'k', 0xff})
	if got != "ok\uFFFD" {
		t.Fatalf("validUTF8String() = %q, want %q", got, "ok\uFFFD")
	}
}

func cutEnv(entry string) (string, string, bool) {
	for i, ch := range entry {
		if ch == '=' {
			return entry[:i], entry[i+1:], true
		}
	}
	return "", "", false
}
