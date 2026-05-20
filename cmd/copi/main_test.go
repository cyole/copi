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

func TestBoolFromArgs(t *testing.T) {
	tests := []struct {
		name     string
		args     []string
		fallback bool
		want     bool
	}{
		{name: "fallback false", args: nil, fallback: false, want: false},
		{name: "fallback true", args: nil, fallback: true, want: true},
		{name: "bare flag", args: []string{"--lan"}, fallback: false, want: true},
		{name: "explicit true", args: []string{"--lan=true"}, fallback: false, want: true},
		{name: "explicit false", args: []string{"--lan=false"}, fallback: true, want: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := boolFromArgs(tt.args, "lan", tt.fallback); got != tt.want {
				t.Fatalf("boolFromArgs() = %v, want %v", got, tt.want)
			}
		})
	}
}
