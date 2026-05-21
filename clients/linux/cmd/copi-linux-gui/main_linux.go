//go:build linux

package main

import (
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/gotk3/gotk3/glib"
	"github.com/gotk3/gotk3/gtk"
)

func main() {
	gtk.Init(nil)

	app, err := newLinuxApp()
	if err != nil {
		log.Fatal(err)
	}
	handleSignals(app)
	app.run()

	gtk.Main()
}

func handleSignals(app *linuxApp) {
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-signals
		glib.IdleAdd(func() {
			app.quit()
		})
	}()
}
