package clipboard

import (
	"sort"
	"strings"
)

func withEnvOverrides(base []string, overrides map[string]string) []string {
	env := make([]string, 0, len(base)+len(overrides))
	for _, entry := range base {
		key, _, ok := strings.Cut(entry, "=")
		if !ok || key == "" {
			env = append(env, entry)
			continue
		}
		if _, replace := overrides[key]; replace {
			continue
		}
		env = append(env, entry)
	}

	keys := make([]string, 0, len(overrides))
	for key := range overrides {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		env = append(env, key+"="+overrides[key])
	}
	return env
}

func validUTF8String(data []byte) string {
	return strings.ToValidUTF8(string(data), "\uFFFD")
}
