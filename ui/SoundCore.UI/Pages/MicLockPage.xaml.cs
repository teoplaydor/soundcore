using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Navigation;
using SoundCore.UI.Localization;
using SoundCore.UI.Services;
using Soundcore.V1;

namespace SoundCore.UI.Pages;

public sealed partial class MicLockPage : Page
{
    private IpcClient? _ipc;
    private bool _suspendCallbacks;

    public MicLockPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _ipc = e.Parameter as IpcClient;
        _ = LoadAsync();
    }

    private async Task LoadAsync()
    {
        if (_ipc is null || !_ipc.IsConnected) return;
        _suspendCallbacks = true;
        try
        {
            // First populate the capture device combobox.
            var devResp = await _ipc.SendAsync(new Request { ListCaptureDevices = new Empty() });
            if (devResp.PayloadCase == Response.PayloadOneofCase.DeviceList)
            {
                DeviceCombo.ItemsSource = devResp.DeviceList.Devices.ToList();
            }
            else
            {
                DeviceCombo.ItemsSource = Array.Empty<AudioDevice>();
            }

            // Then read the current mic-lock config.
            var cfgResp = await _ipc.SendAsync(new Request { GetMicLock = new Empty() });
            if (cfgResp.PayloadCase == Response.PayloadOneofCase.MicLock)
            {
                var c = cfgResp.MicLock;
                EnableSwitch.IsOn = c.Enabled;
                LockMuteSwitch.IsOn = c.AlsoLockMute;
                VolumeSlider.Value = c.LockedVolume >= 0 ? c.LockedVolume * 100.0 : 100.0;
                VolumeText.Text = $"{(int)VolumeSlider.Value}%";
                WhitelistBox.Text = string.Join("\r\n", c.AllowedImageGlobs);

                if (DeviceCombo.ItemsSource is System.Collections.Generic.List<AudioDevice> devs)
                {
                    var match = devs.FirstOrDefault(d => c.Device?.Value == d.Id?.Value);
                    DeviceCombo.SelectedItem = match ?? devs.FirstOrDefault(d => d.IsDefault) ?? devs.FirstOrDefault();
                }
            }
        }
        finally
        {
            _suspendCallbacks = false;
        }
    }

    private void EnableSwitch_OnToggled(object sender, RoutedEventArgs e)
    {
        if (_suspendCallbacks) return;
    }

    private void LockMuteSwitch_OnToggled(object sender, RoutedEventArgs e)
    {
        if (_suspendCallbacks) return;
    }

    private void DeviceCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suspendCallbacks) return;
    }

    private void VolumeSlider_OnValueChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        VolumeText.Text = $"{(int)e.NewValue}%";
    }

    private async void Apply_OnClick(object sender, RoutedEventArgs e)
    {
        if (_ipc is null || !_ipc.IsConnected) return;
        var device = DeviceCombo.SelectedItem as AudioDevice;
        if (EnableSwitch.IsOn && device is null)
        {
            StatusText.Text = Loc.Get("MicLock_NeedDevice");
            return;
        }

        var globs = (WhitelistBox.Text ?? string.Empty)
            .Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(s => s.Trim('\r', ' ', '\t'))
            .Where(s => s.Length > 0)
            .ToList();

        var cfg = new MicLockConfig
        {
            Enabled = EnableSwitch.IsOn,
            Device = device?.Id,
            LockedVolume = (float)(VolumeSlider.Value / 100.0),
            AlsoLockMute = LockMuteSwitch.IsOn,
            RevertImmediately = true,
        };
        cfg.AllowedImageGlobs.AddRange(globs);

        ApplyButton.IsEnabled = false;
        try
        {
            var resp = await _ipc.SendAsync(new Request { SetMicLock = cfg });
            StatusText.Text = resp.PayloadCase == Response.PayloadOneofCase.Error
                ? Loc.Get("Common_ErrorFmt", resp.Error.Message)
                : Loc.Get("Common_Applied");
        }
        catch (Exception ex)
        {
            StatusText.Text = Loc.Get("Common_ErrorFmt", ex.Message);
        }
        finally
        {
            ApplyButton.IsEnabled = true;
        }
    }
}
