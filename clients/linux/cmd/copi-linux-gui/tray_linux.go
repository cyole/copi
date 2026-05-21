//go:build linux

package main

/*
#cgo pkg-config: gtk+-3.0
#include <stdint.h>
#include <stdlib.h>
#include <gtk/gtk.h>

extern void copiTrayIconActivate(uintptr_t id);
extern void copiTrayIconPopupMenu(uintptr_t id, guint button, guint32 activate_time);

#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

static void copi_tray_icon_activate(GtkStatusIcon *icon, gpointer user_data) {
	copiTrayIconActivate((uintptr_t)user_data);
}

static void copi_tray_icon_popup_menu(GtkStatusIcon *icon, guint button, guint32 activate_time, gpointer user_data) {
	copiTrayIconPopupMenu((uintptr_t)user_data, button, activate_time);
}

static GtkStatusIcon *copi_tray_icon_new_from_icon_name(const char *icon_name) {
	return gtk_status_icon_new_from_icon_name(icon_name);
}

static GtkStatusIcon *copi_tray_icon_new_from_file(const char *filename, char **error_message) {
	GError *err = NULL;
	GdkPixbuf *pixbuf = gdk_pixbuf_new_from_file_at_size(filename, 22, 22, &err);
	if (pixbuf == NULL) {
		if (error_message != NULL && err != NULL) {
			*error_message = g_strdup(err->message);
		}
		if (err != NULL) {
			g_error_free(err);
		}
		return NULL;
	}

	GtkStatusIcon *icon = gtk_status_icon_new_from_pixbuf(pixbuf);
	g_object_unref(pixbuf);
	return icon;
}

static void copi_tray_icon_set_title(GtkStatusIcon *icon, const char *title) {
	gtk_status_icon_set_title(icon, title);
}

static void copi_tray_icon_set_tooltip_text(GtkStatusIcon *icon, const char *text) {
	gtk_status_icon_set_tooltip_text(icon, text);
}

static void copi_tray_icon_set_visible(GtkStatusIcon *icon, gboolean visible) {
	gtk_status_icon_set_visible(icon, visible);
}

static gboolean copi_tray_icon_is_embedded(GtkStatusIcon *icon) {
	return gtk_status_icon_is_embedded(icon);
}

static void copi_tray_icon_connect_activate(GtkStatusIcon *icon, uintptr_t id) {
	g_signal_connect(icon, "activate", G_CALLBACK(copi_tray_icon_activate), (gpointer)id);
}

static void copi_tray_icon_connect_popup_menu(GtkStatusIcon *icon, uintptr_t id) {
	g_signal_connect(icon, "popup-menu", G_CALLBACK(copi_tray_icon_popup_menu), (gpointer)id);
}

#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
*/
import "C"
import (
	"fmt"
	"sync"
	"unsafe"
)

type trayIcon struct {
	id          uintptr
	native      *C.GtkStatusIcon
	onActivate  func()
	onPopupMenu func(button uint, activateTime uint32)
}

var trayIcons = struct {
	sync.Mutex
	next uintptr
	byID map[uintptr]*trayIcon
}{
	byID: make(map[uintptr]*trayIcon),
}

func newTrayIconFromIconName(iconName string) (*trayIcon, error) {
	cIconName := C.CString(iconName)
	defer C.free(unsafe.Pointer(cIconName))
	return wrapTrayIcon(C.copi_tray_icon_new_from_icon_name(cIconName))
}

func newTrayIconFromFile(filename string) (*trayIcon, error) {
	cFilename := C.CString(filename)
	defer C.free(unsafe.Pointer(cFilename))

	var cErr *C.char
	icon := C.copi_tray_icon_new_from_file(cFilename, &cErr)
	if icon == nil && cErr != nil {
		defer C.g_free(C.gpointer(unsafe.Pointer(cErr)))
		return nil, fmt.Errorf("load tray icon %s: %s", filename, C.GoString(cErr))
	}
	return wrapTrayIcon(icon)
}

func wrapTrayIcon(native *C.GtkStatusIcon) (*trayIcon, error) {
	if native == nil {
		return nil, fmt.Errorf("create GTK status icon")
	}

	icon := &trayIcon{native: native}
	trayIcons.Lock()
	trayIcons.next++
	icon.id = trayIcons.next
	trayIcons.byID[icon.id] = icon
	trayIcons.Unlock()
	return icon, nil
}

func lookupTrayIcon(id uintptr) *trayIcon {
	trayIcons.Lock()
	defer trayIcons.Unlock()
	return trayIcons.byID[id]
}

func (i *trayIcon) SetTitle(title string) {
	cTitle := C.CString(title)
	defer C.free(unsafe.Pointer(cTitle))
	C.copi_tray_icon_set_title(i.native, cTitle)
}

func (i *trayIcon) SetTooltipText(text string) {
	cText := C.CString(text)
	defer C.free(unsafe.Pointer(cText))
	C.copi_tray_icon_set_tooltip_text(i.native, cText)
}

func (i *trayIcon) SetVisible(visible bool) {
	C.copi_tray_icon_set_visible(i.native, gbool(visible))
}

func (i *trayIcon) IsEmbedded() bool {
	return C.copi_tray_icon_is_embedded(i.native) != 0
}

func (i *trayIcon) ConnectActivate(fn func()) {
	i.onActivate = fn
	C.copi_tray_icon_connect_activate(i.native, C.uintptr_t(i.id))
}

func (i *trayIcon) ConnectPopupMenu(fn func(button uint, activateTime uint32)) {
	i.onPopupMenu = fn
	C.copi_tray_icon_connect_popup_menu(i.native, C.uintptr_t(i.id))
}

func gbool(v bool) C.gboolean {
	if v {
		return C.gboolean(1)
	}
	return C.gboolean(0)
}
