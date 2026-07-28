using System.Windows;
using Companion.Infrastructure.Config;
using Companion.Infrastructure.Paths;

namespace Companion.App;

public partial class SettingsWindow
{
    private readonly JsonConfigStore _store;
    private AppSettings _settings;
    private bool _loaded;

    public SettingsWindow(AppSettings settings, JsonConfigStore store, AppDataPaths paths)
    {
        InitializeComponent();
        _settings = settings;
        _store = store;
        ScaleSlider.Value = settings.PetScale;
        TopmostCheck.IsChecked = settings.Topmost;
        IdleCheck.IsChecked = settings.IdleActionsEnabled;
        ReduceMotionCheck.IsChecked = settings.ReduceMotion;
        DataPathText.Text = paths.Root;
        _loaded = true;
    }

    private async void OnScaleChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (!_loaded)
        {
            return;
        }

        _settings = _settings with { PetScale = e.NewValue };
        await _store.SaveAsync(_settings);
    }

    private async void OnOptionChanged(object sender, RoutedEventArgs e)
    {
        if (!_loaded)
        {
            return;
        }

        _settings = _settings with
        {
            Topmost = TopmostCheck.IsChecked == true,
            IdleActionsEnabled = IdleCheck.IsChecked == true,
            ReduceMotion = ReduceMotionCheck.IsChecked == true
        };
        await _store.SaveAsync(_settings);
    }

    private void CloseWindow(object sender, RoutedEventArgs e) => Close();
}
