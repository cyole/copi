//go:build linux

package clipboard

import (
	"errors"
	"os"
	"os/exec"
	"strings"
)

type linuxBackend struct {
	readArgs  []string
	writeArgs []string
}

var linuxBackends = []linuxBackend{
	{readArgs: []string{"wl-paste", "--no-newline"}, writeArgs: []string{"wl-copy"}},
	{readArgs: []string{"xclip", "-selection", "clipboard", "-out"}, writeArgs: []string{"xclip", "-selection", "clipboard"}},
	{readArgs: []string{"xsel", "--clipboard", "--output"}, writeArgs: []string{"xsel", "--clipboard", "--input"}},
}

func (System) ReadText() (string, error) {
	var errs []error
	for _, backend := range preferredLinuxBackends() {
		out, err := exec.Command(backend.readArgs[0], backend.readArgs[1:]...).Output()
		if err == nil {
			return string(out), nil
		}
		errs = append(errs, err)
	}
	return "", errors.Join(errs...)
}

func (System) WriteText(text string) error {
	var errs []error
	for _, backend := range preferredLinuxBackends() {
		cmd := exec.Command(backend.writeArgs[0], backend.writeArgs[1:]...)
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

func preferredLinuxBackends() []linuxBackend {
	if strings.TrimSpace(os.Getenv("WAYLAND_DISPLAY")) != "" {
		return linuxBackends
	}
	return []linuxBackend{linuxBackends[1], linuxBackends[2], linuxBackends[0]}
}
