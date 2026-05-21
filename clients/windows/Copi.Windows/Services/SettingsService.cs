using System.Text.Json;
using System.Text.Json.Serialization;
using Copi.Windows.Models;

namespace Copi.Windows.Services;

public sealed class SettingsService
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    };

    public string SettingsPath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "Copi",
        "windows-gui.json");

    public AppSettings Load()
    {
        try
        {
            if (!File.Exists(SettingsPath))
            {
                return AppSettings.Default();
            }

            var settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(SettingsPath), JsonOptions)
                ?? AppSettings.Default();
            settings.ApplyDefaults();
            return settings;
        }
        catch
        {
            return AppSettings.Default();
        }
    }

    public void Save(AppSettings settings)
    {
        settings.ApplyDefaults();
        Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
        File.WriteAllText(SettingsPath, JsonSerializer.Serialize(settings, JsonOptions) + Environment.NewLine);
        ApplyStartupPreference(settings);
    }

    public void ApplyStartupPreference(AppSettings settings)
    {
        StartupService.SetEnabled(settings.LaunchAtLogin);
    }
}
