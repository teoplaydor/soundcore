using System;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SoundCore.UI.Localization;
using SoundCore.UI.Services;
using Soundcore.V1;

namespace SoundCore.UI.Pages;

public sealed partial class DevicesPage : Page
{
    private IpcClient? _ipc;

    public DevicesPage()
    {
        InitializeComponent();
        PlaybackPivot.Header = Loc.Get("Devices_Playback");
        CapturePivot.Header = Loc.Get("Devices_Capture");
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _ipc = e.Parameter as IpcClient;
        _ = LoadAsync(DataFlow.Render);
    }

    private void FlowPivot_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (FlowPivot.SelectedItem is PivotItem pi && pi.Tag is string tag)
        {
            _ = LoadAsync(tag == "render" ? DataFlow.Render : DataFlow.Capture);
        }
    }

    private async System.Threading.Tasks.Task LoadAsync(DataFlow flow)
    {
        if (_ipc is null || !_ipc.IsConnected)
        {
            ShowDevices(Array.Empty<AudioDevice>());
            return;
        }

        var request = new Request();
        if (flow == DataFlow.Render)
            request.ListRenderDevices = new Empty();
        else
            request.ListCaptureDevices = new Empty();

        LoadingRing.IsActive = true;
        try
        {
            var response = await _ipc.SendAsync(request);
            ShowDevices(response.PayloadCase == Response.PayloadOneofCase.DeviceList
                ? response.DeviceList.Devices.ToList()
                : (System.Collections.Generic.IReadOnlyList<AudioDevice>)Array.Empty<AudioDevice>());
        }
        catch
        {
            ShowDevices(Array.Empty<AudioDevice>());
        }
        finally
        {
            LoadingRing.IsActive = false;
        }
    }

    private void ShowDevices(System.Collections.Generic.IReadOnlyList<AudioDevice> devices)
    {
        DeviceList.ItemsSource = devices;
        var empty = devices.Count == 0;
        EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        DeviceList.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
    }
}
