using Microsoft.UI.Xaml.Markup;

namespace SoundCore.UI.Localization;

/// <summary>
/// XAML markup extension for localized strings:
///   xmlns:loc="using:SoundCore.UI.Localization"
///   &lt;TextBlock Text="{loc:Str Key=Devices_Title}" /&gt;
///
/// Values are resolved at parse time; pages are re-created on language
/// change (MainWindow re-navigates), which re-resolves every extension.
/// </summary>
[MarkupExtensionReturnType(ReturnType = typeof(string))]
public sealed class StrExtension : MarkupExtension
{
    public string Key { get; set; } = string.Empty;

    protected override object ProvideValue() => Loc.Get(Key);
}
