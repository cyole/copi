using System.Diagnostics;
using Microsoft.Win32;

namespace Copi.Windows.Services;

public static class StartupService
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Copi";

    public static void SetEnabled(bool enabled)
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKey, writable: true)
            ?? Registry.CurrentUser.CreateSubKey(RunKey, writable: true);

        if (!enabled)
        {
            key.DeleteValue(ValueName, throwOnMissingValue: false);
            return;
        }

        var exe = Process.GetCurrentProcess().MainModule?.FileName;
        if (string.IsNullOrWhiteSpace(exe))
        {
            return;
        }
        key.SetValue(ValueName, Quote(exe), RegistryValueKind.String);
    }

    private static string Quote(string path)
    {
        return path.Contains(' ') ? $"\"{path}\"" : path;
    }
}
