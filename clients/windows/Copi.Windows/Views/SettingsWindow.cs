using Copi.Windows.Models;
using Copi.Windows.Services;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Copi.Windows.Views;

public sealed class SettingsWindow : Window
{
    private readonly SettingsService _settingsService;
    private readonly ProcessController _processController;
    private readonly Action<AppSettings> _saveSettings;
    private readonly Action _toggleSync;
    private AppSettings _settings;

    private TextBlock _statusText = null!;
    private Button _toggleButton = null!;
    private RadioButton _relayRadio = null!;
    private RadioButton _lanRadio = null!;
    private StackPanel _relayFields = null!;
    private StackPanel _lanFields = null!;
    private TextBox _relayURL = null!;
    private PasswordBox _accessToken = null!;
    private PasswordBox _syncKey = null!;
    private TextBox _deviceName = null!;
    private TextBox _deviceID = null!;
    private CheckBox _launchAtLogin = null!;

    public SettingsWindow(
        AppSettings settings,
        SettingsService settingsService,
        ProcessController processController,
        Action<AppSettings> saveSettings,
        Action toggleSync)
    {
        _settings = settings.Clone();
        _settingsService = settingsService;
        _processController = processController;
        _saveSettings = saveSettings;
        _toggleSync = toggleSync;

        Title = "设置";
        Content = BuildContent();
        WindowHelpers.ResizeAndCenter(this, 760, 560);
        WindowHelpers.HideInsteadOfClose(this);
        RefreshStatus();
        RefreshModeVisibility();

        _processController.StatusChanged += ProcessStatusChanged;
    }

    private UIElement BuildContent()
    {
        var root = new Grid
        {
            Background = new SolidColorBrush(Colors.White),
            RowDefinitions =
            {
                new RowDefinition { Height = GridLength.Auto },
                new RowDefinition { Height = new GridLength(1, GridUnitType.Star) },
            },
        };

        var header = BuildHeader();
        Grid.SetRow(header, 0);
        root.Children.Add(header);

        var scroll = new ScrollViewer
        {
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Content = BuildForm(),
        };
        Grid.SetRow(scroll, 1);
        root.Children.Add(scroll);
        return root;
    }

    private UIElement BuildHeader()
    {
        var grid = new Grid
        {
            Padding = new Thickness(24, 20, 24, 16),
            ColumnDefinitions =
            {
                new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) },
                new ColumnDefinition { Width = GridLength.Auto },
            },
        };

        var titleStack = new StackPanel { Spacing = 5 };
        titleStack.Children.Add(new TextBlock
        {
            Text = "Copi",
            FontSize = 24,
            FontWeight = FontWeights.SemiBold,
        });
        _statusText = new TextBlock
        {
            Foreground = new SolidColorBrush(ColorHelper.FromArgb(255, 94, 98, 105)),
        };
        titleStack.Children.Add(_statusText);
        Grid.SetColumn(titleStack, 0);
        grid.Children.Add(titleStack);

        _toggleButton = new Button
        {
            MinWidth = 116,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        _toggleButton.Click += (_, _) =>
        {
            SaveFromFields();
            _toggleSync();
            RefreshStatus();
        };
        Grid.SetColumn(_toggleButton, 1);
        grid.Children.Add(_toggleButton);

        return grid;
    }

    private UIElement BuildForm()
    {
        var stack = new StackPanel
        {
            Padding = new Thickness(24, 6, 24, 24),
            Spacing = 16,
        };

        stack.Children.Add(BuildSyncSection());
        stack.Children.Add(BuildDeviceSection());
        stack.Children.Add(BuildBehaviorSection());
        stack.Children.Add(BuildFooter());
        return stack;
    }

    private UIElement BuildSyncSection()
    {
        var section = Section("同步");

        var modeRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 16,
        };
        _relayRadio = new RadioButton { Content = "中转模式", GroupName = "mode", IsChecked = _settings.Mode == SyncMode.Relay };
        _lanRadio = new RadioButton { Content = "局域网", GroupName = "mode", IsChecked = _settings.Mode == SyncMode.Lan };
        _relayRadio.Checked += (_, _) =>
        {
            _settings.Mode = SyncMode.Relay;
            RefreshModeVisibility();
        };
        _lanRadio.Checked += (_, _) =>
        {
            _settings.Mode = SyncMode.Lan;
            RefreshModeVisibility();
        };
        modeRow.Children.Add(_relayRadio);
        modeRow.Children.Add(_lanRadio);
        section.Children.Add(modeRow);

        _relayFields = new StackPanel { Spacing = 10 };
        _relayURL = new TextBox
        {
            Header = "中转服务地址",
            Text = _settings.RelayURL,
            PlaceholderText = "http://127.0.0.1:9527",
        };
        _accessToken = new PasswordBox
        {
            Header = "访问令牌",
            Password = _settings.AccessToken,
        };
        _relayFields.Children.Add(_relayURL);
        _relayFields.Children.Add(_accessToken);
        section.Children.Add(_relayFields);

        _lanFields = new StackPanel { Spacing = 10 };
        var keyGrid = new Grid
        {
            ColumnDefinitions =
            {
                new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) },
                new ColumnDefinition { Width = GridLength.Auto },
            },
            ColumnSpacing = 10,
        };
        _syncKey = new PasswordBox
        {
            Header = "同步密钥",
            Password = _settings.SyncKey,
        };
        Grid.SetColumn(_syncKey, 0);
        keyGrid.Children.Add(_syncKey);
        var generateKey = new Button
        {
            Content = "生成",
            VerticalAlignment = VerticalAlignment.Bottom,
        };
        generateKey.Click += (_, _) => _syncKey.Password = SecretGenerator.Generate();
        Grid.SetColumn(generateKey, 1);
        keyGrid.Children.Add(generateKey);
        _lanFields.Children.Add(keyGrid);
        section.Children.Add(_lanFields);

        return section;
    }

    private UIElement BuildDeviceSection()
    {
        var section = Section("设备");
        _deviceName = new TextBox
        {
            Header = "名称",
            Text = _settings.DeviceName,
        };
        section.Children.Add(_deviceName);

        var idGrid = new Grid
        {
            ColumnDefinitions =
            {
                new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) },
                new ColumnDefinition { Width = GridLength.Auto },
            },
            ColumnSpacing = 10,
        };
        _deviceID = new TextBox
        {
            Header = "ID",
            Text = _settings.DeviceID,
        };
        Grid.SetColumn(_deviceID, 0);
        idGrid.Children.Add(_deviceID);
        var generateID = new Button
        {
            Content = "生成",
            VerticalAlignment = VerticalAlignment.Bottom,
        };
        generateID.Click += (_, _) => _deviceID.Text = "win-" + Guid.NewGuid();
        Grid.SetColumn(generateID, 1);
        idGrid.Children.Add(generateID);
        section.Children.Add(idGrid);
        return section;
    }

    private UIElement BuildBehaviorSection()
    {
        var section = Section("行为");
        _launchAtLogin = new CheckBox
        {
            Content = "登录后自动启动 Copi",
            IsChecked = _settings.LaunchAtLogin,
        };
        section.Children.Add(_launchAtLogin);
        return section;
    }

    private UIElement BuildFooter()
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 10,
        };
        var save = new Button { Content = "保存" };
        save.Click += (_, _) => SaveFromFields();
        row.Children.Add(save);
        return row;
    }

    private static StackPanel Section(string title)
    {
        var panel = new StackPanel
        {
            Spacing = 12,
            Padding = new Thickness(18),
            Background = new SolidColorBrush(ColorHelper.FromArgb(255, 248, 249, 250)),
        };
        panel.Children.Add(new TextBlock
        {
            Text = title,
            FontSize = 16,
            FontWeight = FontWeights.SemiBold,
        });
        return panel;
    }

    private void SaveFromFields()
    {
        _settings.RelayURL = _relayURL.Text.Trim();
        _settings.AccessToken = _accessToken.Password.Trim();
        _settings.SyncKey = _syncKey.Password.Trim();
        _settings.DeviceName = _deviceName.Text.Trim();
        _settings.DeviceID = _deviceID.Text.Trim();
        _settings.LaunchAtLogin = _launchAtLogin.IsChecked == true;
        _settings.ApplyDefaults();
        _settingsService.Save(_settings);
        _saveSettings(_settings.Clone());
        RefreshStatus();
    }

    private void RefreshModeVisibility()
    {
        _relayFields.Visibility = _settings.Mode == SyncMode.Relay ? Visibility.Visible : Visibility.Collapsed;
        _lanFields.Visibility = _settings.Mode == SyncMode.Lan ? Visibility.Visible : Visibility.Collapsed;
    }

    private void RefreshStatus()
    {
        _statusText.Text = $"{_processController.StatusTitle()} · {_settings.ModeTitle()}";
        _toggleButton.Content = _processController.IsRunning ? "停止同步" : "启动同步";
        _toggleButton.IsEnabled = _processController.CanStart || _processController.IsRunning;
    }

    private void ProcessStatusChanged(object? sender, EventArgs e)
    {
        DispatcherQueue.TryEnqueue(RefreshStatus);
    }
}
