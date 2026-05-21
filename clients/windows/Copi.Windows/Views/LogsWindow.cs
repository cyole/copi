using Copi.Windows.Models;
using Copi.Windows.Services;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Copi.Windows.Views;

public sealed class LogsWindow : Window
{
    private readonly ProcessController _processController;
    private TextBox _logText = null!;
    private TextBlock _statusText = null!;

    public LogsWindow(ProcessController processController)
    {
        _processController = processController;
        Title = "日志";
        Content = BuildContent();
        WindowHelpers.ResizeAndCenter(this, 760, 460);
        WindowHelpers.HideInsteadOfClose(this);
        Refresh();

        _processController.LogReceived += LogReceived;
        _processController.StatusChanged += StatusChanged;
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

        var header = new Grid
        {
            Padding = new Thickness(20, 16, 20, 12),
            ColumnDefinitions =
            {
                new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) },
                new ColumnDefinition { Width = GridLength.Auto },
            },
        };
        var title = new StackPanel { Spacing = 4 };
        title.Children.Add(new TextBlock
        {
            Text = "日志",
            FontSize = 20,
            FontWeight = FontWeights.SemiBold,
        });
        _statusText = new TextBlock
        {
            Foreground = new SolidColorBrush(ColorHelper.FromArgb(255, 94, 98, 105)),
        };
        title.Children.Add(_statusText);
        header.Children.Add(title);

        var clear = new Button { Content = "清空" };
        clear.Click += (_, _) =>
        {
            _processController.ClearLogs();
            Refresh();
        };
        Grid.SetColumn(clear, 1);
        header.Children.Add(clear);
        Grid.SetRow(header, 0);
        root.Children.Add(header);

        _logText = new TextBox
        {
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.NoWrap,
            FontFamily = new FontFamily("Consolas"),
            Margin = new Thickness(20, 0, 20, 20),
            VerticalAlignment = VerticalAlignment.Stretch,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        Grid.SetRow(_logText, 1);
        root.Children.Add(_logText);
        return root;
    }

    private void Refresh()
    {
        _statusText.Text = _processController.StatusTitle();
        _logText.Text = string.Join(Environment.NewLine, _processController.Logs.Select(Display));
        _logText.SelectionStart = _logText.Text.Length;
    }

    private static string Display(LogEvent entry)
    {
        return entry.DisplayLine();
    }

    private void LogReceived(object? sender, LogEvent e)
    {
        DispatcherQueue.TryEnqueue(Refresh);
    }

    private void StatusChanged(object? sender, EventArgs e)
    {
        DispatcherQueue.TryEnqueue(Refresh);
    }
}
