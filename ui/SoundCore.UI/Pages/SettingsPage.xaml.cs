using System;
using System.Linq;
using System.Reflection;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SoundCore.UI.Localization;
using SoundCore.UI.Services;

namespace SoundCore.UI.Pages;

public sealed partial class SettingsPage : Page
{
    private sealed record LanguageItem(string? Tag, string Display)
    {
        public override string ToString() => Display;
    }

    private sealed record ThemeItem(string Tag, string Display)
    {
        public override string ToString() => Display;
    }

    private IpcClient? _ipc;
    private bool _initializing;

    public SettingsPage()
    {
        InitializeComponent();
        PopulateCombos();
        VersionText.Text = Loc.Get("Settings_VersionFmt",
            Assembly.GetExecutingAssembly().GetName().Version?.ToString(3) ?? "0.1.0");
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _ipc = e.Parameter as IpcClient;
        UpdateServiceState();
    }

    private void PopulateCombos()
    {
        _initializing = true;
        try
        {
            var languages = new[] { new LanguageItem(null, Loc.Get("Settings_Language_System")) }
                .Concat(Loc.SupportedLanguages.Select(tag => new LanguageItem(tag, Loc.NativeName(tag))))
                .ToList();
            LanguageCombo.ItemsSource = languages;
            LanguageCombo.SelectedItem =
                languages.FirstOrDefault(l => l.Tag == Loc.UserOverride) ?? languages[0];

            var themes = new[]
            {
                new ThemeItem("system", Loc.Get("Settings_Theme_System")),
                new ThemeItem("light", Loc.Get("Settings_Theme_Light")),
                new ThemeItem("dark", Loc.Get("Settings_Theme_Dark")),
            };
            ThemeCombo.ItemsSource = themes;
            ThemeCombo.SelectedItem =
                themes.FirstOrDefault(t => t.Tag == UiSettings.Current.Theme) ?? themes[0];
        }
        finally
        {
            _initializing = false;
        }
    }

    private void UpdateServiceState()
    {
        var connected = _ipc?.IsConnected == true;
        ServiceStateText.Text = Loc.Get(connected ? "Conn_Connected" : "Conn_Disconnected");
        ServiceIcon.Glyph = connected ? "\uE73E" : "\uEA39"; // check mark / error badge
    }

    private void LanguageCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_initializing || LanguageCombo.SelectedItem is not LanguageItem item)
            return;
        Loc.SetLanguage(item.Tag);
        // MainWindow re-navigates on LanguageChanged, which re-creates this
        // page with freshly resolved strings — nothing else to do here.
    }

    private void ThemeCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_initializing || ThemeCombo.SelectedItem is not ThemeItem item)
            return;
        if (App.MainWindow is { } window)
            ThemeHelper.Set(window, item.Tag);
    }
}
