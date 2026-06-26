using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using SoundCore.UI.Localization;
using SoundCore.UI.Pages;
using SoundCore.UI.Services;

namespace SoundCore.UI;

public sealed partial class MainWindow : Window
{
    public IpcClient Ipc { get; } = new IpcClient();

    public MainWindow()
    {
        InitializeComponent();
        Title = "SoundCore";
        ExtendsContentIntoTitleBar = true;

        ApplyLocalizedChrome();
        ThemeHelper.Apply(this);
        Loc.LanguageChanged += OnLanguageChanged;

        Loaded_Async();
        Nav.SelectedItem = Nav.MenuItems[0];

        Closed += (_, _) => Loc.LanguageChanged -= OnLanguageChanged;
    }

    private void OnLanguageChanged(object? sender, EventArgs e)
    {
        ApplyLocalizedChrome();
        // Re-navigate the current page so its parse-time localized strings
        // resolve against the new language.
        if (Nav.SelectedItem is NavigationViewItem { Tag: string tag })
            Navigate(tag, suppressTransition: true);
        else if (Nav.SelectedItem is NavigationViewItemBase)
            Navigate("settings", suppressTransition: true);
    }

    private void ApplyLocalizedChrome()
    {
        NavDevices.Content = Loc.Get("Nav_Devices");
        NavPerApp.Content = Loc.Get("Nav_PerApp");
        NavVst.Content = Loc.Get("Nav_Vst");
        NavMicLock.Content = Loc.Get("Nav_MicLock");
        NavCamera.Content = Loc.Get("Nav_Camera");
    }

    private async void Loaded_Async()
    {
        try
        {
            await Ipc.ConnectAsync();
            ConnectionBanner.IsOpen = false;
        }
        catch (Exception ex)
        {
            ConnectionBanner.Title = Loc.Get("Conn_Failed");
            ConnectionBanner.Message = Loc.Get("Conn_FailedFmt", ex.Message);
            ConnectionBanner.IsOpen = true;
        }
    }

    private void Nav_OnSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.IsSettingsSelected)
        {
            Navigate("settings", suppressTransition: false);
            return;
        }
        if (args.SelectedItem is NavigationViewItem { Tag: string tag })
            Navigate(tag, suppressTransition: false);
    }

    private void Navigate(string tag, bool suppressTransition)
    {
        var pageType = tag switch
        {
            "devices" => typeof(DevicesPage),
            "perapp" => typeof(PerAppPage),
            "vst" => typeof(VstPage),
            "miclock" => typeof(MicLockPage),
            "camera" => typeof(CameraPage),
            "settings" => typeof(SettingsPage),
            _ => typeof(DevicesPage),
        };
        var info = suppressTransition
            ? (NavigationTransitionInfo)new SuppressNavigationTransitionInfo()
            : new EntranceNavigationTransitionInfo();
        ContentFrame.Navigate(pageType, Ipc, info);
    }
}
