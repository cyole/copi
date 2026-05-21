//go:build linux

package main

/*
#cgo pkg-config: ayatana-appindicator3-0.1
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <gtk/gtk.h>
#include <libayatana-appindicator/app-indicator.h>

extern void copiTrayIconActivate(uintptr_t id);
extern void copiTrayIconPopupMenu(uintptr_t id, guint button, guint32 activate_time);

#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

static void copi_appindicator_log_handler(const gchar *log_domain,
                                          GLogLevelFlags log_level,
                                          const gchar *message,
                                          gpointer user_data) {
	if (message != NULL && strstr(message, "libayatana-appindicator is deprecated") != NULL) {
		return;
	}
	g_log_default_handler(log_domain, log_level, message, user_data);
}

static void copi_silence_appindicator_deprecation_warning(void) {
	static gboolean installed = FALSE;
	if (installed) {
		return;
	}
	installed = TRUE;
	g_log_set_handler("libayatana-appindicator",
	                  G_LOG_LEVEL_WARNING,
	                  copi_appindicator_log_handler,
	                  NULL);
}

static AppIndicator *copi_tray_icon_new_from_icon_name(const char *id, const char *icon_name) {
	copi_silence_appindicator_deprecation_warning();
	return app_indicator_new(id, icon_name, APP_INDICATOR_CATEGORY_APPLICATION_STATUS);
}

static AppIndicator *copi_tray_icon_new_with_path(const char *id, const char *icon_name, const char *icon_path) {
	copi_silence_appindicator_deprecation_warning();
	return app_indicator_new_with_path(id, icon_name, APP_INDICATOR_CATEGORY_APPLICATION_STATUS, icon_path);
}

static void copi_tray_icon_set_title(AppIndicator *icon, const char *title) {
	app_indicator_set_title(icon, title);
}

static void copi_tray_icon_set_icon_name(AppIndicator *icon, const char *icon_name) {
	app_indicator_set_icon_full(icon, icon_name, "Copi");
}

static void copi_tray_icon_set_icon_with_path(AppIndicator *icon, const char *icon_name, const char *icon_path) {
	app_indicator_set_icon_theme_path(icon, icon_path);
	app_indicator_set_icon_full(icon, icon_name, "Copi");
}

static void copi_tray_icon_set_visible(AppIndicator *icon, gboolean visible) {
	app_indicator_set_status(icon, visible ? APP_INDICATOR_STATUS_ACTIVE : APP_INDICATOR_STATUS_PASSIVE);
}

static void copi_tray_icon_set_menu(AppIndicator *icon, uintptr_t menu) {
	app_indicator_set_menu(icon, GTK_MENU((gpointer)menu));
}

static void copi_status_icon_activate(GtkStatusIcon *icon, gpointer user_data) {
	copiTrayIconActivate((uintptr_t)user_data);
}

static void copi_status_icon_popup_menu(GtkStatusIcon *icon, guint button, guint32 activate_time, gpointer user_data) {
	copiTrayIconPopupMenu((uintptr_t)user_data, button, activate_time);
}

static GtkStatusIcon *copi_status_icon_new_from_icon_name(const char *icon_name) {
	return gtk_status_icon_new_from_icon_name(icon_name);
}

static GtkStatusIcon *copi_status_icon_new_from_file(const char *filename, char **error_message) {
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

static void copi_status_icon_set_title(GtkStatusIcon *icon, const char *title) {
	gtk_status_icon_set_title(icon, title);
}

static void copi_status_icon_set_from_icon_name(GtkStatusIcon *icon, const char *icon_name) {
	gtk_status_icon_set_from_icon_name(icon, icon_name);
}

static void copi_status_icon_set_from_file(GtkStatusIcon *icon, const char *filename) {
	gtk_status_icon_set_from_file(icon, filename);
}

static void copi_status_icon_set_tooltip_text(GtkStatusIcon *icon, const char *text) {
	gtk_status_icon_set_tooltip_text(icon, text);
}

static void copi_status_icon_set_visible(GtkStatusIcon *icon, gboolean visible) {
	gtk_status_icon_set_visible(icon, visible);
}

static gboolean copi_status_icon_is_embedded(GtkStatusIcon *icon) {
	return gtk_status_icon_is_embedded(icon);
}

static void copi_status_icon_connect_activate(GtkStatusIcon *icon, uintptr_t id) {
	g_signal_connect(icon, "activate", G_CALLBACK(copi_status_icon_activate), (gpointer)id);
}

static void copi_status_icon_connect_popup_menu(GtkStatusIcon *icon, uintptr_t id) {
	g_signal_connect(icon, "popup-menu", G_CALLBACK(copi_status_icon_popup_menu), (gpointer)id);
}

static void copi_status_icon_popup_at_icon(GtkStatusIcon *icon, GtkMenu *menu, guint button, guint32 activate_time) {
	gtk_menu_popup(menu, NULL, NULL, gtk_status_icon_position_menu, icon, button, activate_time);
}

#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
*/
import "C"
import (
	"fmt"
	"path/filepath"
	"strings"
	"sync"
	"unsafe"

	"github.com/gotk3/gotk3/gtk"
)

type trayKind int

const (
	trayKindIndicator trayKind = iota
	trayKindStatusIcon
)

type trayIcon struct {
	id          uintptr
	kind        trayKind
	indicator   *C.AppIndicator
	statusIcon  *C.GtkStatusIcon
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
	if !runningWaylandSession() {
		cIconName := C.CString(iconName)
		defer C.free(unsafe.Pointer(cIconName))
		return wrapStatusIcon(C.copi_status_icon_new_from_icon_name(cIconName))
	}

	cID := C.CString(appID)
	defer C.free(unsafe.Pointer(cID))
	cIconName := C.CString(iconName)
	defer C.free(unsafe.Pointer(cIconName))
	return wrapTrayIcon(C.copi_tray_icon_new_from_icon_name(cID, cIconName))
}

func newTrayIconFromFile(filename string) (*trayIcon, error) {
	if !runningWaylandSession() {
		cFilename := C.CString(filename)
		defer C.free(unsafe.Pointer(cFilename))

		var cErr *C.char
		icon := C.copi_status_icon_new_from_file(cFilename, &cErr)
		if icon == nil && cErr != nil {
			defer C.g_free(C.gpointer(unsafe.Pointer(cErr)))
			return nil, fmt.Errorf("load tray icon %s: %s", filename, C.GoString(cErr))
		}
		return wrapStatusIcon(icon)
	}

	iconName := strings.TrimSuffix(filepath.Base(filename), filepath.Ext(filename))
	iconPath := filepath.Dir(filename)

	cID := C.CString(appID)
	defer C.free(unsafe.Pointer(cID))
	cIconName := C.CString(iconName)
	defer C.free(unsafe.Pointer(cIconName))
	cIconPath := C.CString(iconPath)
	defer C.free(unsafe.Pointer(cIconPath))
	return wrapTrayIcon(C.copi_tray_icon_new_with_path(cID, cIconName, cIconPath))
}

func trayIconNameAndPath(filename string) (string, string) {
	return strings.TrimSuffix(filepath.Base(filename), filepath.Ext(filename)), filepath.Dir(filename)
}

func wrapTrayIcon(native *C.AppIndicator) (*trayIcon, error) {
	if native == nil {
		return nil, fmt.Errorf("create app indicator")
	}
	return &trayIcon{
		kind:      trayKindIndicator,
		indicator: native,
	}, nil
}

func wrapStatusIcon(native *C.GtkStatusIcon) (*trayIcon, error) {
	if native == nil {
		return nil, fmt.Errorf("create GTK status icon")
	}

	icon := &trayIcon{
		kind:       trayKindStatusIcon,
		statusIcon: native,
	}
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
	switch i.kind {
	case trayKindIndicator:
		C.copi_tray_icon_set_title(i.indicator, cTitle)
	case trayKindStatusIcon:
		C.copi_status_icon_set_title(i.statusIcon, cTitle)
	}
}

func (i *trayIcon) SetIconName(iconName string) {
	cIconName := C.CString(iconName)
	defer C.free(unsafe.Pointer(cIconName))
	switch i.kind {
	case trayKindIndicator:
		C.copi_tray_icon_set_icon_name(i.indicator, cIconName)
	case trayKindStatusIcon:
		C.copi_status_icon_set_from_icon_name(i.statusIcon, cIconName)
	}
}

func (i *trayIcon) SetIconFromFile(filename string) {
	if strings.TrimSpace(filename) == "" {
		return
	}
	switch i.kind {
	case trayKindIndicator:
		iconName, iconPath := trayIconNameAndPath(filename)
		cIconName := C.CString(iconName)
		defer C.free(unsafe.Pointer(cIconName))
		cIconPath := C.CString(iconPath)
		defer C.free(unsafe.Pointer(cIconPath))
		C.copi_tray_icon_set_icon_with_path(i.indicator, cIconName, cIconPath)
	case trayKindStatusIcon:
		cFilename := C.CString(filename)
		defer C.free(unsafe.Pointer(cFilename))
		C.copi_status_icon_set_from_file(i.statusIcon, cFilename)
	}
}

func (i *trayIcon) SetTooltipText(text string) {
	cText := C.CString(text)
	defer C.free(unsafe.Pointer(cText))
	switch i.kind {
	case trayKindIndicator:
		C.copi_tray_icon_set_title(i.indicator, cText)
	case trayKindStatusIcon:
		C.copi_status_icon_set_tooltip_text(i.statusIcon, cText)
	}
}

func (i *trayIcon) SetVisible(visible bool) {
	switch i.kind {
	case trayKindIndicator:
		C.copi_tray_icon_set_visible(i.indicator, gbool(visible))
	case trayKindStatusIcon:
		C.copi_status_icon_set_visible(i.statusIcon, gbool(visible))
	}
}

func (i *trayIcon) IsEmbedded() bool {
	switch i.kind {
	case trayKindIndicator:
		return true
	case trayKindStatusIcon:
		return C.copi_status_icon_is_embedded(i.statusIcon) != 0
	default:
		return false
	}
}

func (i *trayIcon) SetMenu(menu *gtk.Menu) {
	if i.kind == trayKindIndicator {
		C.copi_tray_icon_set_menu(i.indicator, C.uintptr_t(menu.Native()))
	}
}

func (i *trayIcon) ConnectActivate(fn func()) {
	i.onActivate = fn
	if i.kind == trayKindStatusIcon {
		C.copi_status_icon_connect_activate(i.statusIcon, C.uintptr_t(i.id))
	}
}

func (i *trayIcon) ConnectPopupMenu(fn func(button uint, activateTime uint32)) {
	i.onPopupMenu = fn
	if i.kind == trayKindStatusIcon {
		C.copi_status_icon_connect_popup_menu(i.statusIcon, C.uintptr_t(i.id))
	}
}

func (i *trayIcon) PopupMenu(menu *gtk.Menu, button uint, activateTime uint32) {
	if i.kind == trayKindStatusIcon {
		C.copi_status_icon_popup_at_icon(i.statusIcon, (*C.GtkMenu)(unsafe.Pointer(menu.Native())), C.guint(button), C.guint32(activateTime))
	}
}

func gbool(v bool) C.gboolean {
	if v {
		return C.gboolean(1)
	}
	return C.gboolean(0)
}
