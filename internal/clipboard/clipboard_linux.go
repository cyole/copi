//go:build linux

package clipboard

import (
	"errors"
	"os"
	"os/exec"
	"strings"
)

const utf8TextMIME = "text/plain;charset=utf-8"

var (
	waylandReadCommands = [][]string{
		{"wl-paste", "--no-newline", "--type", utf8TextMIME},
		{"wl-paste", "--no-newline", "--type", "text/plain"},
		{"wl-paste", "--no-newline"},
	}
	waylandWriteCommands = [][]string{
		{"wl-copy", "--type", utf8TextMIME},
	}
	x11ReadCommands = [][]string{
		{"xclip", "-selection", "clipboard", "-target", "UTF8_STRING", "-out"},
		{"xclip", "-selection", "clipboard", "-out"},
		{"xsel", "--clipboard", "--output"},
	}
	x11WriteCommands = [][]string{
		{"xclip", "-selection", "clipboard", "-target", "UTF8_STRING"},
		{"xsel", "--clipboard", "--input"},
	}
)

func (System) ReadText() (string, error) {
	var errs []error
	for _, args := range preferredLinuxReadCommands() {
		out, err := utf8LinuxCommand(args).Output()
		if err == nil {
			return validUTF8String(out), nil
		}
		errs = append(errs, err)
	}
	return "", errors.Join(errs...)
}

func (System) WriteText(text string) error {
	var errs []error
	for _, args := range preferredLinuxWriteCommands() {
		cmd := utf8LinuxCommand(args)
		stdin, err := cmd.StdinPipe()
		if err != nil {
			errs = append(errs, err)
			continue
		}
		if err := cmd.Start(); err != nil {
			errs = append(errs, err)
			continue
		}
		if _, err := stdin.Write([]byte(text)); err != nil {
			_ = stdin.Close()
			_ = cmd.Wait()
			errs = append(errs, err)
			continue
		}
		if err := stdin.Close(); err != nil {
			_ = cmd.Wait()
			errs = append(errs, err)
			continue
		}
		if err := cmd.Wait(); err != nil {
			errs = append(errs, err)
			continue
		}
		return nil
	}
	return errors.Join(errs...)
}

func preferredLinuxReadCommands() [][]string {
	if strings.TrimSpace(os.Getenv("WAYLAND_DISPLAY")) != "" {
		return appendCommandLists(waylandReadCommands, x11ReadCommands)
	}
	return appendCommandLists(x11ReadCommands, waylandReadCommands)
}

func preferredLinuxWriteCommands() [][]string {
	if strings.TrimSpace(os.Getenv("WAYLAND_DISPLAY")) != "" {
		return appendCommandLists(waylandWriteCommands, x11WriteCommands)
	}
	return appendCommandLists(x11WriteCommands, waylandWriteCommands)
}

func appendCommandLists(first, second [][]string) [][]string {
	commands := make([][]string, 0, len(first)+len(second))
	commands = append(commands, first...)
	commands = append(commands, second...)
	return commands
}

func utf8LinuxCommand(args []string) *exec.Cmd {
	cmd := exec.Command(args[0], args[1:]...)
	cmd.Env = withEnvOverrides(os.Environ(), map[string]string{
		"LANG":     "C.UTF-8",
		"LC_CTYPE": "C.UTF-8",
	})
	return cmd
}
