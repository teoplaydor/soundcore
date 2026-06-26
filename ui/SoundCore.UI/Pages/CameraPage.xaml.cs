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

public sealed partial class CameraPage : Page
{
    private IpcClient? _ipc;

    public CameraPage()
    {
        InitializeComponent();
        ApplySharingModeGuidance();
    }

    /// <summary>
    /// Picks the camera-sharing strategy message based on the OS build.
    /// Windows 11 24H2+ (build 26100) has the native Multi-App Camera mode,
    /// which shares the real camera with no virtual device — we point the
    /// user at the system toggle. Older Windows can't do native multi-app
    /// sharing for arbitrary apps, so we run in compatibility mode via the
    /// SoundCore camera device. See docs/camera-sharing-research.md.
    /// </summary>
    private void ApplySharingModeGuidance()
    {
        var build = Environment.OSVersion.Version.Build;
        if (build >= 26100)
        {
            ModeInfoBar.Severity = InfoBarSeverity.Success;
            ModeInfoBar.Title = "Native multi-app camera available";
            ModeInfoBar.Message =
                "This Windows version can share the real camera with several apps at once. " +
                "Enable “Let multiple apps use your camera” in Windows camera settings; " +
                "no virtual camera is needed.";
            OpenCameraSettings.Visibility = Visibility.Visible;
        }
        else if (build >= 22000)
        {
            ModeInfoBar.Severity = InfoBarSeverity.Informational;
            ModeInfoBar.Title = "Compatibility mode";
            ModeInfoBar.Message =
                "Native multi-app camera sharing arrives in Windows 11 24H2. On this build, " +
                "SoundCore shares the camera through its own camera device, which DirectShow " +
                "apps (Zoom, Chrome, OBS, Telegram) can use simultaneously.";
        }
        else
        {
            ModeInfoBar.Severity = InfoBarSeverity.Informational;
            ModeInfoBar.Title = "Compatibility mode (Windows 10)";
            ModeInfoBar.Message =
                "Windows 10 has no native multi-app camera. SoundCore shares the camera through " +
                "its own camera device, usable by DirectShow apps (Zoom, Chrome, OBS, Telegram) " +
                "at the same time.";
        }
    }

    private async void OpenCameraSettings_OnClick(object sender, RoutedEventArgs e)
    {
        try { await Windows.System.Launcher.LaunchUriAsync(new Uri("ms-settings:camera")); }
        catch { /* settings app unavailable */ }
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _ipc = e.Parameter as IpcClient;
        _ = LoadAsync();
    }

    private async Task LoadAsync()
    {
        if (_ipc is null || !_ipc.IsConnected) return;

        var camResp = await _ipc.SendAsync(new Request { ListCameras = new Empty() });
        if (camResp.PayloadCase == Response.PayloadOneofCase.CameraList)
        {
            var cams = camResp.CameraList.Cameras.ToList();
            SourceCombo.ItemsSource = cams;
            EmptyState.Visibility = cams.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        }

        var cfgResp = await _ipc.SendAsync(new Request { GetCameraMultiplex = new Empty() });
        if (cfgResp.PayloadCase == Response.PayloadOneofCase.CameraMultiplex)
        {
            var c = cfgResp.CameraMultiplex;
            EnableSwitch.IsOn = c.Enabled;
            if (SourceCombo.ItemsSource is System.Collections.Generic.List<CameraSource> srcs &&
                srcs.FirstOrDefault(s => s.SymbolicLink == c.SourceSymbolicLink) is { } pick)
            {
                SourceCombo.SelectedItem = pick;
            }
        }
    }

    private async void Apply_OnClick(object sender, RoutedEventArgs e)
    {
        if (_ipc is null || !_ipc.IsConnected) return;
        var src = SourceCombo.SelectedItem as CameraSource;

        var (w, h, fpsNum, fpsDen) = ParseResolution(
            (ResolutionCombo.SelectedItem as ComboBoxItem)?.Tag as string ?? "1280x720@30");

        var cfg = new CameraMultiplexConfig
        {
            Enabled = EnableSwitch.IsOn,
            SourceSymbolicLink = src?.SymbolicLink ?? string.Empty,
            PreferredFormat = new CameraFormat
            {
                Width = (uint)w,
                Height = (uint)h,
                FrameRateNum = (uint)fpsNum,
                FrameRateDen = (uint)fpsDen,
                Subtype = "NV12",
            },
        };

        ApplyButton.IsEnabled = false;
        try
        {
            var resp = await _ipc.SendAsync(new Request { SetCameraMultiplex = cfg });
            StatusText.Text = resp.PayloadCase == Response.PayloadOneofCase.Error
                ? Loc.Get("Common_ErrorFmt", resp.Error.Message)
                : Loc.Get("Common_Applied");
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
        finally
        {
            ApplyButton.IsEnabled = true;
        }
    }

    private static (int w, int h, int fpsNum, int fpsDen) ParseResolution(string tag)
    {
        try
        {
            var atParts = tag.Split('@');
            var sizeParts = atParts[0].Split('x');
            return (int.Parse(sizeParts[0]), int.Parse(sizeParts[1]), int.Parse(atParts[1]), 1);
        }
        catch
        {
            return (1280, 720, 30, 1);
        }
    }
}
