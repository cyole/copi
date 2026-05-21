using System.Text.Json;

namespace Copi.Windows.Models;

public sealed class LogEvent
{
    public DateTimeOffset ReceivedAt { get; init; } = DateTimeOffset.Now;
    public string Source { get; init; } = "gui";
    public string Level { get; init; } = "info";
    public string Type { get; init; } = "event";
    public string Message { get; init; } = "";
    public string Raw { get; init; } = "";

    public string DisplayLine()
    {
        var message = string.IsNullOrWhiteSpace(Message) ? Raw : Message;
        return $"{ReceivedAt:HH:mm:ss} [{Level}] {Type}  {message}";
    }

    public static LogEvent FromProcessLine(string line, string source)
    {
        try
        {
            using var doc = JsonDocument.Parse(line);
            var root = doc.RootElement;
            return new LogEvent
            {
                Source = source,
                Level = Read(root, "level", "info"),
                Type = Read(root, "type", source),
                Message = Read(root, "message", line),
                Raw = line,
            };
        }
        catch (JsonException)
        {
            return new LogEvent
            {
                Source = source,
                Level = source == "stderr" ? "error" : "info",
                Type = source,
                Message = line,
                Raw = line,
            };
        }
    }

    private static string Read(JsonElement root, string name, string fallback)
    {
        return root.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? fallback
            : fallback;
    }
}
