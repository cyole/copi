//go:build linux

package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
	"syscall"
	"time"

	"github.com/gotk3/gotk3/glib"
)

type processStatus string

const (
	statusStopped  processStatus = "stopped"
	statusStarting processStatus = "starting"
	statusRunning  processStatus = "running"
	statusStopping processStatus = "stopping"
	statusFailed   processStatus = "failed"
)

type logEvent struct {
	ReceivedAt time.Time
	Source     string
	Level      string
	Type       string
	Message    string
	Raw        string
}

type processController struct {
	app    *linuxApp
	cmd    *exec.Cmd
	status processStatus
}

func newProcessController(app *linuxApp) *processController {
	return &processController{
		app:    app,
		status: statusStopped,
	}
}

func (p *processController) isRunning() bool {
	return p.cmd != nil && p.status == statusRunning
}

func (p *processController) canStart() bool {
	return p.status == statusStopped || p.status == statusFailed
}

func (p *processController) statusTitle() string {
	switch p.status {
	case statusStarting:
		return "启动中"
	case statusRunning:
		if p.cmd != nil && p.cmd.Process != nil {
			return fmt.Sprintf("运行中 · %d", p.cmd.Process.Pid)
		}
		return "运行中"
	case statusStopping:
		return "停止中"
	case statusFailed:
		return "启动失败"
	default:
		return "已停止"
	}
}

func (p *processController) start(settings appSettings) {
	if !p.canStart() {
		return
	}
	if !settings.runnable() {
		p.fail("请补全运行参数")
		return
	}

	cliPath, err := resolveCLIPath()
	if err != nil {
		p.fail(err.Error())
		return
	}

	args := []string{"client"}
	if settings.Mode == modeLAN {
		args = append(args, "--lan")
	} else {
		args = append(args, "--relay", strings.TrimSpace(settings.RelayURL))
	}
	if secret := strings.TrimSpace(settings.secret()); secret != "" {
		args = append(args, "--token", secret)
	}
	args = append(args,
		"--id", settings.DeviceID,
		"--name", settings.DeviceName,
		"--log-format", "json",
	)

	cmd := exec.Command(cliPath, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		p.fail(err.Error())
		return
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		p.fail(err.Error())
		return
	}

	p.setStatus(statusStarting)
	if err := cmd.Start(); err != nil {
		p.fail(err.Error())
		return
	}

	p.cmd = cmd
	p.setStatus(statusRunning)
	p.app.appendLog(logEvent{
		ReceivedAt: time.Now(),
		Source:     "gui",
		Level:      "info",
		Type:       "started",
		Message:    "copi client 已启动",
	})

	go p.readPipe(stdout, "stdout")
	go p.readPipe(stderr, "stderr")
	go p.wait(cmd)
}

func (p *processController) stop() {
	if p.cmd == nil || p.cmd.Process == nil {
		p.cmd = nil
		p.setStatus(statusStopped)
		return
	}

	cmd := p.cmd
	p.setStatus(statusStopping)
	_ = syscall.Kill(-cmd.Process.Pid, syscall.SIGTERM)

	go func() {
		time.Sleep(3 * time.Second)
		glib.IdleAdd(func() {
			if p.cmd == cmd && p.status == statusStopping {
				_ = syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
			}
		})
	}()
}

func (p *processController) readPipe(pipe interface {
	Read([]byte) (int, error)
}, source string) {
	scanner := bufio.NewScanner(pipe)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		captured := line
		glib.IdleAdd(func() {
			p.app.consumeProcessLine(captured, source)
		})
	}
	if err := scanner.Err(); err != nil && !strings.Contains(err.Error(), "file already closed") {
		glib.IdleAdd(func() {
			p.app.appendLog(logEvent{
				ReceivedAt: time.Now(),
				Source:     source,
				Level:      "error",
				Type:       source,
				Message:    err.Error(),
			})
		})
	}
}

func (p *processController) wait(cmd *exec.Cmd) {
	err := cmd.Wait()
	exitCode := 0
	if err != nil {
		exitCode = 1
		if exitErr, ok := err.(*exec.ExitError); ok {
			exitCode = exitErr.ExitCode()
		}
	}

	glib.IdleAdd(func() {
		if p.cmd != cmd {
			return
		}
		p.cmd = nil
		if exitCode == 0 || p.status == statusStopping {
			p.setStatus(statusStopped)
		} else {
			p.fail(fmt.Sprintf("copi exited with status %d", exitCode))
		}
	})
}

func (p *processController) fail(message string) {
	p.app.appendLog(logEvent{
		ReceivedAt: time.Now(),
		Source:     "gui",
		Level:      "error",
		Type:       "failed",
		Message:    message,
	})
	p.setStatus(statusFailed)
}

func (p *processController) setStatus(status processStatus) {
	p.status = status
	p.app.refreshStatus()
}

func parseLogLine(line, source string) logEvent {
	event := logEvent{
		ReceivedAt: time.Now(),
		Source:     source,
		Level:      "info",
		Type:       source,
		Message:    line,
		Raw:        line,
	}
	if source == "stderr" {
		event.Level = "error"
	}

	var payload map[string]any
	if err := json.Unmarshal([]byte(line), &payload); err != nil {
		return event
	}
	if level, ok := payload["level"].(string); ok && level != "" {
		event.Level = level
	}
	if eventType, ok := payload["type"].(string); ok && eventType != "" {
		event.Type = eventType
	}
	if message, ok := payload["message"].(string); ok && message != "" {
		event.Message = message
	}
	return event
}
