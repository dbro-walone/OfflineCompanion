using System.Windows;
using Companion.Application.Events;
using Companion.Infrastructure.Config;
using Companion.Infrastructure.Paths;

namespace Companion.App;

public partial class SettingsWindow
{
    private readonly JsonConfigStore _store;
    private readonly IEventBus _eventBus;
    private bool _loaded;

    public SettingsWindow(
        AppSettings settings,
        JsonConfigStore store,
        AppDataPaths paths,
        IEventBus eventBus)
    {
        InitializeComponent();
        _store = store;
        _eventBus = eventBus;
        ScaleSlider.Value = settings.PetScale;
        TopmostCheck.IsChecked = settings.Topmost;
        IdleCheck.IsChecked = settings.IdleActionsEnabled;
        ReduceMotionCheck.IsChecked = settings.ReduceMotion;
        DataPathText.Text = paths.Root;
        SaveButton.IsEnabled = false;
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (_loaded)
        {
            return;
        }

        _loaded = true;
        try
        {
            var settings = await _store.LoadAsync();
            ScaleSlider.Value = settings.PetScale;
            TopmostCheck.IsChecked = settings.Topmost;
            IdleCheck.IsChecked = settings.IdleActionsEnabled;
            ReduceMotionCheck.IsChecked = settings.ReduceMotion;
            SaveButton.IsEnabled = true;
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                this,
                $"读取设置失败：{ex.Message}",
                "设置",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Close();
        }
    }

    private async void SaveAndApply(object sender, RoutedEventArgs e)
    {
        SaveButton.IsEnabled = false;
        try
        {
            var currentSettings = await _store.LoadAsync();
            var updatedSettings = currentSettings with
            {
                PetScale = ScaleSlider.Value,
                Topmost = TopmostCheck.IsChecked == true,
                IdleActionsEnabled = IdleCheck.IsChecked == true,
                ReduceMotion = ReduceMotionCheck.IsChecked == true
            };

            await _store.SaveAsync(updatedSettings);
            _eventBus.Publish(new SettingsChanged(updatedSettings, DateTimeOffset.Now));
            Close();
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                this,
                $"保存设置失败：{ex.Message}",
                "设置",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            SaveButton.IsEnabled = true;
        }
    }

    private void CancelChanges(object sender, RoutedEventArgs e) => Close();
}
