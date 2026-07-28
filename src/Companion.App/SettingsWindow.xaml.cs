using System.Windows;
using System.Windows.Controls;
using Companion.Application.Events;
using Companion.Infrastructure.Config;
using Companion.Infrastructure.Paths;

namespace Companion.App;

public partial class SettingsWindow
{
    private readonly JsonConfigStore _store;
    private readonly IEventBus _eventBus;
    private bool _loaded;
    private bool _themeSaved;
    private string _originalTheme;

    public SettingsWindow(
        AppSettings settings,
        JsonConfigStore store,
        AppDataPaths paths,
        IEventBus eventBus)
    {
        InitializeComponent();
        _store = store;
        _eventBus = eventBus;
        _originalTheme = ThemeManager.Normalize(settings.Theme);
        ScaleSlider.Value = settings.PetScale;
        TopmostCheck.IsChecked = settings.Topmost;
        IdleCheck.IsChecked = settings.IdleActionsEnabled;
        ReduceMotionCheck.IsChecked = settings.ReduceMotion;
        SelectTheme(settings.Theme);
        DataPathText.Text = paths.Root;
        SaveButton.IsEnabled = false;
        Closing += (_, _) =>
        {
            if (!_themeSaved)
            {
                ThemeManager.Apply(_originalTheme);
            }
        };
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
            _originalTheme = ThemeManager.Normalize(settings.Theme);
            ScaleSlider.Value = settings.PetScale;
            TopmostCheck.IsChecked = settings.Topmost;
            IdleCheck.IsChecked = settings.IdleActionsEnabled;
            ReduceMotionCheck.IsChecked = settings.ReduceMotion;
            SelectTheme(settings.Theme);
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
                ReduceMotion = ReduceMotionCheck.IsChecked == true,
                Theme = SelectedTheme()
            };

            await _store.SaveAsync(updatedSettings);
            ThemeManager.Apply(updatedSettings.Theme);
            _eventBus.Publish(new SettingsChanged(updatedSettings, DateTimeOffset.Now));
            _themeSaved = true;
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

    private void OnThemeSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_loaded)
        {
            ThemeManager.Apply(SelectedTheme());
        }
    }

    private string SelectedTheme() =>
        ThemeCombo.SelectedItem is ComboBoxItem { Tag: string theme }
            ? ThemeManager.Normalize(theme)
            : ThemeManager.Dark;

    private void SelectTheme(string? theme)
    {
        var normalized = ThemeManager.Normalize(theme);
        ThemeCombo.SelectedIndex = normalized == ThemeManager.Light ? 1 : 0;
    }
}
