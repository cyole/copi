//go:build !darwin && !linux && !windows

package clipboard

import "errors"

func (System) ReadText() (string, error) {
	return "", errors.New("system clipboard is not supported on this platform yet")
}

func (System) WriteText(string) error {
	return errors.New("system clipboard is not supported on this platform yet")
}
