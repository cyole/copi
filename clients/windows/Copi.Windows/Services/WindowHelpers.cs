using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using System.Runtime.InteropServices;
using WinRT.Interop;

namespace Copi.Windows.Services;

public static class WindowHelpers
{
    private const int SwHide = 0;
    private const int SwShow = 5;

    public static void ResizeAndCenter(Window window, int width, int height)
    {
        var appWindow = GetAppWindow(window);
        if (appWindow == null)
        {
            return;
        }

        var display = DisplayArea.GetFromWindowId(appWindow.Id, DisplayAreaFallback.Primary);
        var work = display.WorkArea;
        var x = work.X + Math.Max(0, (work.Width - width) / 2);
        var y = work.Y + Math.Max(0, (work.Height - height) / 2);
        appWindow.MoveAndResize(new global::Windows.Graphics.RectInt32(x, y, width, height));
    }

    public static void HideInsteadOfClose(Window window)
    {
        var appWindow = GetAppWindow(window);
        if (appWindow == null)
        {
            return;
        }

        appWindow.Closing += (_, args) =>
        {
            args.Cancel = true;
            Hide(window);
        };
    }

    public static void Show(Window window)
    {
        window.Activate();
        var hwnd = WindowNative.GetWindowHandle(window);
        if (hwnd != IntPtr.Zero)
        {
            _ = ShowWindow(hwnd, SwShow);
            _ = SetForegroundWindow(hwnd);
        }
    }

    private static void Hide(Window window)
    {
        var hwnd = WindowNative.GetWindowHandle(window);
        if (hwnd != IntPtr.Zero)
        {
            _ = ShowWindow(hwnd, SwHide);
        }
    }

    private static AppWindow? GetAppWindow(Window window)
    {
        var hwnd = WindowNative.GetWindowHandle(window);
        if (hwnd == IntPtr.Zero)
        {
            return null;
        }
        var id = Win32Interop.GetWindowIdFromWindow(hwnd);
        return AppWindow.GetFromWindowId(id);
    }

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);
}
