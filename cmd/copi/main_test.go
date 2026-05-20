package main

import "testing"

func TestConfigPathFromArgs(t *testing.T) {
	tests := []struct {
		name string
		args []string
		want string
	}{
		{name: "empty", args: nil, want: ""},
		{name: "separate", args: []string{"--config", "copi.json"}, want: "copi.json"},
		{name: "equals", args: []string{"--config=./copi.json"}, want: "./copi.json"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := configPathFromArgs(tt.args); got != tt.want {
				t.Fatalf("configPathFromArgs() = %q, want %q", got, tt.want)
			}
		})
	}
}
