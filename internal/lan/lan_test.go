package lan

import (
	"net"
	"testing"
)

func TestSplitListenAddr(t *testing.T) {
	tests := []struct {
		name     string
		addr     string
		wantHost string
		wantPort string
	}{
		{name: "host port", addr: "127.0.0.1:9528", wantHost: "127.0.0.1", wantPort: "9528"},
		{name: "wildcard", addr: "0.0.0.0:9528", wantHost: "0.0.0.0", wantPort: "9528"},
		{name: "random wildcard", addr: "0.0.0.0:0", wantHost: "0.0.0.0", wantPort: "0"},
		{name: "colon port", addr: ":9528", wantHost: "", wantPort: "9528"},
		{name: "bare port", addr: "9528", wantHost: "", wantPort: "9528"},
		{name: "ipv6 wildcard", addr: "[::]:9528", wantHost: "::", wantPort: "9528"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			host, port, err := splitListenAddr(tt.addr)
			if err != nil {
				t.Fatalf("splitListenAddr() error = %v", err)
			}
			if host != tt.wantHost || port != tt.wantPort {
				t.Fatalf("splitListenAddr() = (%q, %q), want (%q, %q)", host, port, tt.wantHost, tt.wantPort)
			}
		})
	}
}

func TestIsWildcardHost(t *testing.T) {
	tests := map[string]bool{
		"":          true,
		"0.0.0.0":   true,
		"::":        true,
		"[::]":      true,
		"127.0.0.1": false,
		"10.0.0.2":  false,
	}

	for host, want := range tests {
		if got := isWildcardHost(host); got != want {
			t.Fatalf("isWildcardHost(%q) = %v, want %v", host, got, want)
		}
	}
}

func TestListenPeerHTTPAssignsRandomPort(t *testing.T) {
	listener, actual, err := listenPeerHTTP("127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()

	host, port, err := splitListenAddr(actual)
	if err != nil {
		t.Fatal(err)
	}
	if host != "127.0.0.1" {
		t.Fatalf("host = %q, want 127.0.0.1", host)
	}
	if port == "0" || port == "" {
		t.Fatalf("port = %q, want assigned port", port)
	}
}

func TestIPv4FromAddr(t *testing.T) {
	ip, ipnet, err := net.ParseCIDR("192.168.1.10/24")
	if err != nil {
		t.Fatal(err)
	}
	ipnet.IP = ip
	ipaddr := &net.IPAddr{IP: net.ParseIP("10.0.0.5")}

	if got := ipv4FromAddr(ipnet); got == nil || got.String() != "192.168.1.10" {
		t.Fatalf("ipv4FromAddr(IPNet) = %v", got)
	}
	if got := ipv4FromAddr(ipaddr); got == nil || got.String() != "10.0.0.5" {
		t.Fatalf("ipv4FromAddr(IPAddr) = %v", got)
	}
	if got := ipv4FromAddr(&net.IPAddr{IP: net.ParseIP("2001:db8::1")}); got != nil {
		t.Fatalf("ipv4FromAddr(IPv6) = %v, want nil", got)
	}
}
