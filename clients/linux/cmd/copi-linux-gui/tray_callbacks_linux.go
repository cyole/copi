//go:build linux

package main

/*
#include <stdint.h>
#include <glib.h>
*/
import "C"

//export copiTrayIconActivate
func copiTrayIconActivate(id C.uintptr_t) {
	icon := lookupTrayIcon(uintptr(id))
	if icon != nil && icon.onActivate != nil {
		icon.onActivate()
	}
}

//export copiTrayIconPopupMenu
func copiTrayIconPopupMenu(id C.uintptr_t, button C.guint, activateTime C.guint32) {
	icon := lookupTrayIcon(uintptr(id))
	if icon != nil && icon.onPopupMenu != nil {
		icon.onPopupMenu(uint(button), uint32(activateTime))
	}
}
