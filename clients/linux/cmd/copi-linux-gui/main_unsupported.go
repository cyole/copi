//go:build !linux

package main

import (
	"fmt"
	"runtime"
)

func main() {
	fmt.Printf("copi-linux-gui only runs on Linux, not %s\n", runtime.GOOS)
}
