//go:build linux

package main

import (
	"log"

	"github.com/gotk3/gotk3/gtk"
)

func main() {
	gtk.Init(nil)

	app, err := newLinuxApp()
	if err != nil {
		log.Fatal(err)
	}
	app.run()

	gtk.Main()
}
