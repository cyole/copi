namespace Copi.Windows.Services;

public static class CliPathResolver
{
    public static string Resolve()
    {
        foreach (var candidate in Candidates())
        {
            if (!string.IsNullOrWhiteSpace(candidate) && File.Exists(candidate))
            {
                return candidate;
            }
        }

        throw new FileNotFoundException("找不到 Copi 同步核心，请使用 clients/windows/run.ps1 启动，或安装完整 Windows 包。");
    }

    private static IEnumerable<string> Candidates()
    {
        yield return Environment.GetEnvironmentVariable("COPI_CLI");
        yield return Environment.GetEnvironmentVariable("COPI_CLI_PATH");

        var baseDir = AppContext.BaseDirectory;
        yield return Path.Combine(baseDir, "copi.exe");
        yield return Path.Combine(baseDir, "..", "copi.exe");
        yield return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Copi", "copi.exe");

        var path = Environment.GetEnvironmentVariable("PATH") ?? "";
        foreach (var dir in path.Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries))
        {
            yield return Path.Combine(dir.Trim(), "copi.exe");
        }
    }
}
