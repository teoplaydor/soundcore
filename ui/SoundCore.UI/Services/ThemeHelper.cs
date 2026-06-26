using Microsoft.UI.Xaml;

namespace SoundCore.UI.Services;

/// <summary>Applies the persisted theme choice to a window's root element.</summary>
public static class ThemeHelper
{
    public static ElementTheme FromSetting(string setting) => setting switch
    {
        "light" => ElementTheme.Light,
        "dark" => ElementTheme.Dark,
        _ => ElementTheme.Default,
    };

    public static void Apply(Window window)
    {
        if (window.Content is FrameworkElement root)
            root.RequestedTheme = FromSetting(UiSettings.Current.Theme);
    }

    public static void Set(Window window, string setting)
    {
        UiSettings.Current.Theme = setting;
        UiSettings.Save();
        Apply(window);
    }
}
