using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SoundCore.UI.Localization;
using SoundCore.UI.Services;
using Soundcore.V1;

namespace SoundCore.UI.Pages;

public sealed partial class VstPage : Page
{
    private IpcClient? _ipc;

    public VstPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _ipc = e.Parameter as IpcClient;
        _ = LoadAsync(rescan: false);
    }

    private async Task LoadAsync(bool rescan)
    {
        if (_ipc is null || !_ipc.IsConnected) return;
        ScanProgress.IsActive = true;
        RescanButton.IsEnabled = false;
        StatusText.Text = rescan ? Loc.Get("Vst_Scanning") : Loc.Get("Common_Loading");
        try
        {
            var req = new Request();
            if (rescan) req.RescanVstPlugins = new Empty();
            else req.ListVstPlugins = new Empty();
            var response = await _ipc.SendAsync(req);
            if (response.PayloadCase == Response.PayloadOneofCase.VstPluginList)
            {
                var plugins = response.VstPluginList.Plugins.ToList();
                PluginList.ItemsSource = plugins;
                StatusText.Text = Loc.Get("Vst_CountFmt", plugins.Count);
                var empty = plugins.Count == 0;
                EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
                PluginList.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
            }
            else if (response.PayloadCase == Response.PayloadOneofCase.Error)
            {
                StatusText.Text = Loc.Get("Common_ErrorFmt", response.Error.Message);
            }
        }
        catch (Exception ex)
        {
            StatusText.Text = Loc.Get("Common_ErrorFmt", ex.Message);
        }
        finally
        {
            ScanProgress.IsActive = false;
            RescanButton.IsEnabled = true;
        }
    }

    private void Rescan_OnClick(object sender, RoutedEventArgs e) => _ = LoadAsync(rescan: true);
}
