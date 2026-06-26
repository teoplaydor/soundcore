using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SoundCore.UI.Localization;
using SoundCore.UI.Services;
using Soundcore.V1;

namespace SoundCore.UI.Pages;

public sealed partial class PerAppPage : Page
{
    private IpcClient? _ipc;
    private List<Process> _all = new();

    public PerAppPage()
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
        try
        {
            var response = await _ipc.SendAsync(new Request { ListProcesses = new Empty() });
            if (response.PayloadCase == Response.PayloadOneofCase.ProcessList)
            {
                _all = response.ProcessList.Processes.ToList();
                ApplyFilter();
            }
        }
        catch (Exception ex)
        {
            _all = new List<Process>();
            ApplyFilter();
            System.Diagnostics.Debug.WriteLine($"ListProcesses failed: {ex.Message}");
        }
    }

    private void ApplyFilter()
    {
        var q = SearchBox.Text?.Trim() ?? string.Empty;
        var items = string.IsNullOrEmpty(q)
            ? (System.Collections.Generic.IReadOnlyList<Process>)_all
            : _all.Where(p => p.ImageName.Contains(q, StringComparison.OrdinalIgnoreCase)).ToList();
        ProcessList.ItemsSource = items;
        var empty = items.Count == 0;
        EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        ProcessList.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
    }

    private void SearchBox_OnTextChanged(object sender, TextChangedEventArgs e) => ApplyFilter();
    private void Refresh_OnClick(object sender, RoutedEventArgs e) => _ = LoadAsync();
}
