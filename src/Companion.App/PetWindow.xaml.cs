using System.ComponentModel;
using System.Windows;
using System.Windows.Input;
using Companion.Application.Events;
using Companion.Infrastructure.Config;
using Companion.Infrastructure.Paths;
using Companion.Packages.Models;
using Companion.Packages.Validation;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Win32;

namespace Companion.App;

public partial class PetWindow
{
    private readonly IServiceProvider _services;
    private readonly JsonConfigStore _configStore;
    private readonly IEventBus _eventBus;
    private AppSettings _settings;
    private Point _mouseDown;
    private bool _allowClose;

    public PetWindow(
        IServiceProvider services,
        AppDataPaths paths,
        JsonConfigStore configStore,
        AppSettings settings,
        ManifestValidator validator,
        IEventBus eventBus)
    {
        InitializeComponent();
        _services = services;
        _configStore = configStore;
        _settings = settings;
        _eventBus = eventBus;
        Topmost = settings.Topmost;
        Width *= settings.PetScale;
        Height *= settings.PetScale;
        if (settings.PetLeft is not null && settings.PetTop is not null)
        {
            Left = settings.PetLeft.Value;
            Top = settings.PetTop.Value;
        }

        LoadCharacter(paths, settings.CurrentCharacterId, validator);
        _eventBus.Subscribe<ActionRequested>(OnActionRequested);
        SourceInitialized += (_, _) => MonitorPlacement.ClampAndDetectEdge(this);
        SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;
    }

    private void LoadCharacter(AppDataPaths paths, string id, ManifestValidator validator)
    {
        try
        {
            var candidates = Directory.Exists(paths.Characters)
                ? Directory.EnumerateFiles(paths.Characters, "manifest.json", SearchOption.AllDirectories)
                : [];
            var manifestPath = candidates.FirstOrDefault(path =>
            {
                try
                {
                    return validator.Load(Path.GetDirectoryName(path)!).Id == id;
                }
                catch
                {
                    return false;
                }
            });

            if (manifestPath is null)
            {
                ShowFallback();
                return;
            }

            var root = Path.GetDirectoryName(manifestPath)!;
            var validation = validator.ValidateDirectory(root);
            if (!validation.IsValid || validator.Load(root) is not CharacterManifest manifest)
            {
                ShowFallback();
                return;
            }

            var idle = LoadAnimation(root, manifest.Actions[manifest.DefaultAction]);
            var atlasPath = Path.GetFullPath(Path.Combine(
                root,
                Path.GetDirectoryName(manifest.Actions[manifest.DefaultAction]) ?? string.Empty,
                idle.Atlas));
            Sprite.AtlasPath = atlasPath;
            Sprite.FrameWidth = manifest.Frame.Width;
            Sprite.FrameHeight = manifest.Frame.Height;
            Sprite.Frames = ExpandFrames(idle);
            Sprite.Fps = idle.Fps;

            // Give the sprite a moment to load the atlas; if it fails, show fallback
            // (BitmapImage loads synchronously with CacheOption.OnLoad)
            if (!File.Exists(atlasPath))
            {
                ShowFallback();
            }
        }
        catch
        {
            ShowFallback();
        }
    }

    private void ShowFallback()
    {
        Fallback.Visibility = Visibility.Visible;
        Sprite.Visibility = Visibility.Collapsed;
    }

    private static AnimationDefinition LoadAnimation(string root, string relativePath)
    {
        var json = File.ReadAllText(Path.Combine(root, relativePath));
        return System.Text.Json.JsonSerializer.Deserialize<AnimationDefinition>(
                   json,
                   new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web)
                   {
                       Converters = { new System.Text.Json.Serialization.JsonStringEnumConverter(
                           System.Text.Json.JsonNamingPolicy.KebabCaseLower) }
                   }) ??
               throw new InvalidDataException($"动画定义无效：{relativePath}");
    }

    private static string ExpandFrames(AnimationDefinition animation)
    {
        var segments = new[] { animation.Segments.Entry, animation.Segments.Loop, animation.Segments.Exit }
            .Where(x => x is not null)
            .Cast<AnimationSegment>();
        return string.Join(",", segments.SelectMany(segment =>
            Enumerable.Range(segment.Start, segment.End - segment.Start + 1)
                .SelectMany(frame => Enumerable.Repeat(frame, Math.Max(1, segment.Repeat)))));
    }

    private void OnActionRequested(ActionRequested message)
    {
        Dispatcher.Invoke(() =>
        {
            var frames = message.ActionId switch
            {
                "clicked" => new[] { 4, 5 },
                "celebrate" => new[] { 7, 7, 3 },
                var action when action.StartsWith("reminder.", StringComparison.Ordinal) => new[] { 6, 6, 3 },
                "focus" => new[] { 1, 2 },
                "relax" => new[] { 1, 3 },
                _ => new[] { 0, 1, 2, 3 }
            };
            Sprite.Play(frames, 6);
        });
    }

    private void OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        _mouseDown = e.GetPosition(this);
        if (e.ButtonState == MouseButtonState.Pressed)
        {
            try
            {
                DragMove();
            }
            catch (InvalidOperationException)
            {
                // Mouse was released before WPF entered the native move loop.
            }
        }
    }

    private async void OnMouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        var current = e.GetPosition(this);
        var distance = (current - _mouseDown).Length;
        _settings = _settings with { PetLeft = Left, PetTop = Top };
        var edge = MonitorPlacement.ClampAndDetectEdge(this);
        if (edge is not null && _settings.EdgeActionsEnabled)
        {
            _eventBus.Publish(new ActionRequested(edge, DateTimeOffset.Now));
        }

        _settings = _settings with { PetLeft = Left, PetTop = Top };
        await _configStore.SaveAsync(_settings);
        if (distance < 6)
        {
            _eventBus.Publish(new ActionRequested("clicked", DateTimeOffset.Now));
        }
    }

    private void OpenTodos(object sender, RoutedEventArgs e)
    {
        var window = _services.GetRequiredService<TodoWindow>();
        window.Owner = this;
        window.Show();
    }

    private void OpenReminder(object sender, RoutedEventArgs e)
    {
        var window = _services.GetRequiredService<ReminderWindow>();
        window.Owner = this;
        window.Show();
    }

    private void OpenTimer(object sender, RoutedEventArgs e)
    {
        var window = _services.GetRequiredService<TimerWindow>();
        window.Owner = this;
        window.Show();
    }

    private void OpenSettings(object sender, RoutedEventArgs e)
    {
        var window = _services.GetRequiredService<SettingsWindow>();
        window.Owner = this;
        window.Show();
    }

    private void OpenPackages(object sender, RoutedEventArgs e)
    {
        var window = _services.GetRequiredService<PackageManagerWindow>();
        window.Owner = this;
        window.Show();
    }

    private void ExitApplication(object sender, RoutedEventArgs e)
    {
        _allowClose = true;
        System.Windows.Application.Current.Shutdown();
    }

    private void OnClosing(object? sender, CancelEventArgs e)
    {
        if (!_allowClose)
        {
            e.Cancel = true;
            Hide();
            return;
        }

        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
    }

    private void OnDisplaySettingsChanged(object? sender, EventArgs e)
    {
        Dispatcher.Invoke(() => MonitorPlacement.ClampAndDetectEdge(this));
    }
}
