using System.Collections.ObjectModel;
using System.Diagnostics;
using Copi.Windows.Models;

namespace Copi.Windows.Services;

public sealed class ProcessController : IDisposable
{
    private const int LogLimit = 500;
    private Process? _process;

    public event EventHandler? StatusChanged;
    public event EventHandler<LogEvent>? LogReceived;

    public ObservableCollection<LogEvent> Logs { get; } = new();
    public ProcessStatus Status { get; private set; } = ProcessStatus.Stopped;
    public bool IsRunning => Status == ProcessStatus.Running && _process is { HasExited: false };
    public bool CanStart => Status is ProcessStatus.Stopped or ProcessStatus.Failed;

    public string StatusTitle()
    {
        return Status switch
        {
            ProcessStatus.Starting => "启动中",
            ProcessStatus.Running => _process is { HasExited: false } ? $"运行中 · {_process.Id}" : "运行中",
            ProcessStatus.Stopping => "停止中",
            ProcessStatus.Failed => "启动失败",
            _ => "已停止",
        };
    }

    public void Start(AppSettings settings)
    {
        if (!CanStart)
        {
            return;
        }
        if (!settings.IsRunnable)
        {
            Fail("请补全运行参数");
            return;
        }

        string executable;
        try
        {
            executable = CliPathResolver.Resolve();
        }
        catch (Exception ex)
        {
            Fail(ex.Message);
            return;
        }

        var startInfo = new ProcessStartInfo(executable)
        {
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            WorkingDirectory = Path.GetDirectoryName(executable) ?? AppContext.BaseDirectory,
        };
        foreach (var arg in BuildArguments(settings))
        {
            startInfo.ArgumentList.Add(arg);
        }

        var process = new Process
        {
            StartInfo = startInfo,
            EnableRaisingEvents = true,
        };
        process.OutputDataReceived += (_, e) => ConsumeLine(e.Data, "stdout");
        process.ErrorDataReceived += (_, e) => ConsumeLine(e.Data, "stderr");
        process.Exited += (_, _) => OnExited(process);

        SetStatus(ProcessStatus.Starting);
        try
        {
            if (!process.Start())
            {
                Fail("同步核心启动失败");
                return;
            }
            _process = process;
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();
            SetStatus(ProcessStatus.Running);
            Append(new LogEvent
            {
                Source = "gui",
                Type = "started",
                Message = "Copi 同步已启动",
            });
        }
        catch (Exception ex)
        {
            process.Dispose();
            Fail(ex.Message);
        }
    }

    public void Stop()
    {
        if (_process == null)
        {
            SetStatus(ProcessStatus.Stopped);
            return;
        }

        SetStatus(ProcessStatus.Stopping);
        try
        {
            if (!_process.HasExited)
            {
                _process.Kill(entireProcessTree: true);
            }
        }
        catch (Exception ex)
        {
            Fail(ex.Message);
        }
    }

    public void ClearLogs()
    {
        Logs.Clear();
    }

    private static IEnumerable<string> BuildArguments(AppSettings settings)
    {
        yield return "client";
        if (settings.Mode == SyncMode.Lan)
        {
            yield return "--lan";
        }
        else
        {
            yield return "--relay";
            yield return settings.RelayURL.Trim();
        }

        if (!string.IsNullOrWhiteSpace(settings.Secret))
        {
            yield return "--token";
            yield return settings.Secret.Trim();
        }

        yield return "--id";
        yield return settings.DeviceID.Trim();
        yield return "--name";
        yield return settings.DeviceName.Trim();
        yield return "--log-format";
        yield return "json";
    }

    private void ConsumeLine(string? line, string source)
    {
        if (string.IsNullOrWhiteSpace(line))
        {
            return;
        }
        Append(LogEvent.FromProcessLine(line.Trim(), source));
    }

    private void OnExited(Process process)
    {
        if (!ReferenceEquals(_process, process))
        {
            return;
        }

        var stoppedByUser = Status == ProcessStatus.Stopping;
        var exitCode = 0;
        try
        {
            exitCode = process.ExitCode;
        }
        catch
        {
            // Ignore races while Windows finalizes process state.
        }

        _process = null;
        process.Dispose();

        if (stoppedByUser || exitCode == 0)
        {
            SetStatus(ProcessStatus.Stopped);
            return;
        }
        Fail($"同步核心已退出，状态码 {exitCode}");
    }

    private void Fail(string message)
    {
        Append(new LogEvent
        {
            Source = "gui",
            Level = "error",
            Type = "failed",
            Message = message,
        });
        _process = null;
        SetStatus(ProcessStatus.Failed);
    }

    private void Append(LogEvent entry)
    {
        App.Current.DispatcherQueue.TryEnqueue(() =>
        {
            Logs.Add(entry);
            while (Logs.Count > LogLimit)
            {
                Logs.RemoveAt(0);
            }
            LogReceived?.Invoke(this, entry);
        });
    }

    private void SetStatus(ProcessStatus status)
    {
        Status = status;
        App.Current.DispatcherQueue.TryEnqueue(() =>
        {
            StatusChanged?.Invoke(this, EventArgs.Empty);
        });
    }

    public void Dispose()
    {
        try
        {
            if (_process is { HasExited: false })
            {
                _process.Kill(entireProcessTree: true);
            }
        }
        catch
        {
            // Best effort shutdown during application exit.
        }
        _process?.Dispose();
    }
}
