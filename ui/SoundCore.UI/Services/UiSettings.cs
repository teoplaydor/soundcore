using System;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace SoundCore.UI.Services;

/// <summary>
/// UI-local preferences (language, theme). Persisted to
/// %APPDATA%\SoundCore\ui-settings.json — separate from the service's
/// %ProgramData% config, because these are per-user presentation choices.
/// </summary>
public sealed class UiSettings
{
    /// <summary>BCP-47 tag, or null to follow the OS display language.</summary>
    [JsonPropertyName("language")]
    public string? Language { get; set; }

    /// <summary>"system" | "light" | "dark"</summary>
    [JsonPropertyName("theme")]
    public string Theme { get; set; } = "system";

    private static readonly string FilePath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "SoundCore", "ui-settings.json");

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
    };

    private static UiSettings? _current;

    public static UiSettings Current => _current ??= Load();

    private static UiSettings Load()
    {
        try
        {
            if (File.Exists(FilePath))
                return JsonSerializer.Deserialize<UiSettings>(File.ReadAllText(FilePath)) ?? new UiSettings();
        }
        catch
        {
            // Corrupt settings are not worth crashing the app over.
        }
        return new UiSettings();
    }

    public static void Save()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(Current, JsonOptions));
        }
        catch
        {
            // Best-effort persistence.
        }
    }
}
