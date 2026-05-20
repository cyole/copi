//go:build windows

package clipboard

import (
	"os/exec"
	"strings"
)

func (System) ReadText() (string, error) {
	out, err := exec.Command("powershell.exe", "-NoProfile", "-Command", "Get-Clipboard -Raw").Output()
	if err != nil {
		return "", err
	}
	return strings.TrimRight(string(out), "\r\n"), nil
}

func (System) WriteText(text string) error {
	cmd := exec.Command("powershell.exe", "-NoProfile", "-Command", "[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8; Set-Clipboard")
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
