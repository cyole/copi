//go:build darwin

package clipboard

import (
	"os"
	"os/exec"
	"strings"
)

func (System) ReadText() (string, error) {
	out, err := utf8DarwinCommand("pbpaste", "-Prefer", "txt").Output()
	if err != nil {
		return "", err
	}
	return strings.TrimRight(validUTF8String(out), "\x00"), nil
}

func (System) WriteText(text string) error {
	cmd := utf8DarwinCommand("pbcopy")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return err
	}
	if err := cmd.Start(); err != nil {
		return err
	}
	if _, err := stdin.Write([]byte(text)); err != nil {
		_ = stdin.Close()
		_ = cmd.Wait()
		return err
	}
	if err := stdin.Close(); err != nil {
		_ = cmd.Wait()
		return err
	}
	return cmd.Wait()
}

func utf8DarwinCommand(name string, args ...string) *exec.Cmd {
	cmd := exec.Command(name, args...)
	cmd.Env = withEnvOverrides(os.Environ(), map[string]string{
		"LANG":     "en_US.UTF-8",
		"LC_ALL":   "en_US.UTF-8",
		"LC_CTYPE": "UTF-8",
	})
	return cmd
}
