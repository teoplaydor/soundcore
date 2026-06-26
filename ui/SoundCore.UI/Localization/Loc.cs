using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Microsoft.Windows.ApplicationModel.Resources;

namespace SoundCore.UI.Localization;

/// <summary>
/// Central localization service. Reads strings from the compiled PRI
/// (Strings/&lt;lang&gt;/Resources.resw) through MRT Core with an explicit
/// resource context, so runtime language switching works in unpackaged
/// (WindowsPackageType=None) builds where PrimaryLanguageOverride is
/// unreliable.
/// </summary>
public static class Loc
{
    /// <summary>BCP-47 tags we ship resources for. First entry is the fallback.</summary>
    public static readonly IReadOnlyList<string> SupportedLanguages = new[]
    {
        "en-US", "ru-RU", "uk-UA", "de-DE", "fr-FR", "es-ES", "zh-CN",
    };

    private static readonly ResourceManager Manager = new();
    private static ResourceContext _context = MakeContext(ResolveInitialLanguage());
    private static string _language = ResolveInitialLanguage();

    /// <summary>Raised after the active language changes; UI re-applies strings.</summary>
    public static event EventHandler? LanguageChanged;

    /// <summary>Active BCP-47 tag, always one of <see cref="SupportedLanguages"/>.</summary>
    public static string Language => _language;

    /// <summary>
    /// Effective language requested by the user: a supported tag, or null
    /// when following the OS display language.
    /// </summary>
    public static string? UserOverride { get; private set; } = Services.UiSettings.Current.Language;

    public static void SetLanguage(string? bcp47OrNullForSystem)
    {
        UserOverride = Normalize(bcp47OrNullForSystem);
        Services.UiSettings.Current.Language = UserOverride;
        Services.UiSettings.Save();

        var resolved = UserOverride ?? MatchSystem();
        if (resolved == _language)
        {
            // Still notify: callers expect a refresh after picking "System".
            LanguageChanged?.Invoke(null, EventArgs.Empty);
            return;
        }

        _language = resolved;
        _context = MakeContext(resolved);
        LanguageChanged?.Invoke(null, EventArgs.Empty);
    }

    /// <summary>Look up a localized string by resw key. Returns the key itself when missing.</summary>
    public static string Get(string key)
    {
        try
        {
            var candidate = Manager.MainResourceMap.TryGetValue($"Resources/{key}", _context);
            var value = candidate?.ValueAsString;
            if (!string.IsNullOrEmpty(value))
                return value;
        }
        catch
        {
            // Missing PRI entry or malformed key — fall through to the key.
        }
        return key;
    }

    /// <summary>Look up and format a localized string.</summary>
    public static string Get(string key, params object[] args)
    {
        var pattern = Get(key);
        try
        {
            return string.Format(CultureInfo.CurrentCulture, pattern, args);
        }
        catch (FormatException)
        {
            return pattern;
        }
    }

    /// <summary>Human-readable native name of a supported language tag.</summary>
    public static string NativeName(string tag)
    {
        try
        {
            var name = new CultureInfo(tag).NativeName;
            return char.ToUpper(name[0], new CultureInfo(tag)) + name[1..];
        }
        catch (CultureNotFoundException)
        {
            return tag;
        }
    }

    private static string ResolveInitialLanguage()
        => Normalize(Services.UiSettings.Current.Language) ?? MatchSystem();

    /// <summary>Best supported match for the OS display language.</summary>
    private static string MatchSystem()
    {
        var system = CultureInfo.CurrentUICulture.Name; // e.g. "ru-RU"
        return Normalize(system) ?? SupportedLanguages[0];
    }

    /// <summary>Map an arbitrary tag onto a supported one (exact, then by primary subtag).</summary>
    private static string? Normalize(string? tag)
    {
        if (string.IsNullOrWhiteSpace(tag))
            return null;

        var exact = SupportedLanguages.FirstOrDefault(
            l => l.Equals(tag, StringComparison.OrdinalIgnoreCase));
        if (exact is not null)
            return exact;

        var primary = tag.Split('-')[0];
        return SupportedLanguages.FirstOrDefault(
            l => l.Split('-')[0].Equals(primary, StringComparison.OrdinalIgnoreCase));
    }

    private static ResourceContext MakeContext(string language)
    {
        var ctx = Manager.CreateResourceContext();
        ctx.QualifierValues["Language"] = language;
        return ctx;
    }
}
