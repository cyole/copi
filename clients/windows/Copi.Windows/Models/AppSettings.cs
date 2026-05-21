using Copi.Windows.Services;

namespace Copi.Windows.Models;

public sealed class AppSettings
{
    public SyncMode Mode { get; set; } = SyncMode.Relay;
    public string DeviceName { get; set; } = "";
    public string DeviceID { get; set; } = "";
    public string RelayURL { get; set; } = "http://127.0.0.1:9527";
    public string AccessToken { get; set; } = "";
    public string SyncKey { get; set; } = "";
    public bool LaunchAtLogin { get; set; }

    public static AppSettings Default()
    {
        return new AppSettings
        {
            Mode = SyncMode.Relay,
            DeviceName = Environment.MachineName,
            DeviceID = "win-" + Guid.NewGuid(),
            RelayURL = "http://127.0.0.1:9527",
            AccessToken = "",
            SyncKey = SecretGenerator.Generate(),
            LaunchAtLogin = false,
        };
    }

    public string Secret => Mode == SyncMode.Lan ? SyncKey : AccessToken;

    public bool IsRunnable => Mode == SyncMode.Lan || !string.IsNullOrWhiteSpace(RelayURL);

    public string ModeTitle() => Mode == SyncMode.Lan ? "局域网" : "中转模式";

    public AppSettings Clone()
    {
        return new AppSettings
        {
            Mode = Mode,
            DeviceName = DeviceName,
            DeviceID = DeviceID,
            RelayURL = RelayURL,
            AccessToken = AccessToken,
            SyncKey = SyncKey,
            LaunchAtLogin = LaunchAtLogin,
        };
    }

    public void ApplyDefaults()
    {
        if (string.IsNullOrWhiteSpace(DeviceName))
        {
            DeviceName = Environment.MachineName;
        }
        if (string.IsNullOrWhiteSpace(DeviceID))
        {
            DeviceID = "win-" + Guid.NewGuid();
        }
        if (string.IsNullOrWhiteSpace(RelayURL))
        {
            RelayURL = "http://127.0.0.1:9527";
        }
        if (string.IsNullOrWhiteSpace(SyncKey))
        {
            SyncKey = SecretGenerator.Generate();
        }
    }
}
