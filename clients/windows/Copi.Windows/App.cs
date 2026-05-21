using Copi.Windows.Models;
using Copi.Windows.Services;
using Copi.Windows.Views;
using Microsoft.UI.Xaml;
using Forms = System.Windows.Forms;

namespace Copi.Windows;

public partial class App : Application
{
    private readonly SettingsService _settingsService = new();
    private readonly ProcessController _processController = new();
    private AppSettings _settings = AppSettings.Default();
    private Forms.NotifyIcon? _trayIcon;
    private Forms.ToolStripMenuItem? _statusItem;
    private Forms.ToolStripMenuItem? _modeItem;
    private Forms.ToolStripMenuItem? _toggleItem;
    private SettingsWindow? _settingsWindow;
    private LogsWindow? _logsWindow;

    [STAThread]
    public static void Main()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        Application.Start(context =>
        {
            _ = context;
            _ = new App();
        });
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _settings = _settingsService.Load();
        _settingsService.ApplyStartupPreference(_settings);

        _processController.StatusChanged += ProcessStatusChanged;
        _processController.LogReceived += ProcessLogReceived;

        BuildTray();
        ShowSettings();
    }

    private void BuildTray()
    {
        _trayIcon = new Forms.NotifyIcon
        {
            Text = "Copi",
            Icon = TrayIconFactory.Create(_processController.Status),
            Visible = true,
        };
        _trayIcon.DoubleClick += (_, _) => ShowSettings();

        _statusItem = new Forms.ToolStripMenuItem(_processController.StatusTitle()) { Enabled = false };
        _modeItem = new Forms.ToolStripMenuItem(_settings.ModeTitle()) { Enabled = false };
        _toggleItem = new Forms.ToolStripMenuItem("启动同步");
        _toggleItem.Click += (_, _) => ToggleSync();

        var logsItem = new Forms.ToolStripMenuItem("日志...");
        logsItem.Click += (_, _) => ShowLogs();
        var settingsItem = new Forms.ToolStripMenuItem("设置...");
        settingsItem.Click += (_, _) => ShowSettings();
        var quitItem = new Forms.ToolStripMenuItem("退出 Copi");
        quitItem.Click += (_, _) => Quit();

        _trayIcon.ContextMenuStrip = new Forms.ContextMenuStrip();
        _trayIcon.ContextMenuStrip.Items.Add(_statusItem);
        _trayIcon.ContextMenuStrip.Items.Add(_modeItem);
        _trayIcon.ContextMenuStrip.Items.Add(new Forms.ToolStripSeparator());
        _trayIcon.ContextMenuStrip.Items.Add(_toggleItem);
        _trayIcon.ContextMenuStrip.Items.Add(logsItem);
        _trayIcon.ContextMenuStrip.Items.Add(settingsItem);
        _trayIcon.ContextMenuStrip.Items.Add(new Forms.ToolStripSeparator());
        _trayIcon.ContextMenuStrip.Items.Add(quitItem);
        RefreshTray();
    }

    private void ToggleSync()
    {
        if (_processController.IsRunning)
        {
            _processController.Stop();
            return;
        }

        SaveSettings(_settings);
        _processController.Start(_settings.Clone());
    }

    private void ShowSettings()
    {
        if (_settingsWindow == null)
        {
            _settingsWindow = new SettingsWindow(_settings, _settingsService, _processController, SaveSettings, ToggleSync);
        }
        WindowHelpers.Show(_settingsWindow);
    }

    private void ShowLogs()
    {
        if (_logsWindow == null)
        {
            _logsWindow = new LogsWindow(_processController);
        }
        WindowHelpers.Show(_logsWindow);
    }

    private void SaveSettings(AppSettings settings)
    {
        _settings = settings.Clone();
        _settingsService.Save(_settings);
        RefreshTray();
    }

    private void ProcessStatusChanged(object? sender, EventArgs e)
    {
        DispatcherQueue.TryEnqueue(RefreshTray);
    }

    private void ProcessLogReceived(object? sender, LogEvent e)
    {
        DispatcherQueue.TryEnqueue(RefreshTray);
    }

    private void RefreshTray()
    {
        if (_trayIcon != null)
        {
            _trayIcon.Text = "Copi - " + _processController.StatusTitle();
            _trayIcon.Icon = TrayIconFactory.Create(_processController.Status);
        }
        if (_statusItem != null)
        {
            _statusItem.Text = _processController.StatusTitle();
        }
        if (_modeItem != null)
        {
            _modeItem.Text = _settings.ModeTitle();
        }
        if (_toggleItem != null)
        {
            _toggleItem.Text = _processController.IsRunning ? "停止同步" : "启动同步";
            _toggleItem.Enabled = _processController.CanStart || _processController.IsRunning;
        }
    }

    private void Quit()
    {
        _processController.Dispose();
        _trayIcon?.Dispose();
        Environment.Exit(0);
    }
}
