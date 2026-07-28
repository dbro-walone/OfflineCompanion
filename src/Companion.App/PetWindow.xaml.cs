using System.ComponentModel;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
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
    private static readonly int[] IdleFrames = [0, 1, 2, 3];
    private static readonly int[] ReactionFrames = [4, 5];

    private readonly IServiceProvider _services;
    private readonly JsonConfigStore _configStore;
    private readonly IEventBus _eventBus;
    private readonly DispatcherTimer _idleActionTimer;
    private AppSettings _settings;
    private Point _mouseDown;
    private Storyboard? _hoverStoryboard;
    private bool _allowClose;
    private bool _dragOccurred;
    private bool _isDragging;
    private bool _suppressNextClick;
    private int _sharinganState;

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
        _idleActionTimer = new DispatcherTimer(DispatcherPriority.Background);
        _idleActionTimer.Tick += OnIdleActionTimerTick;

        Topmost = settings.Topmost;
        Width *= settings.PetScale;
        Height *= settings.PetScale;
        if (settings.PetLeft is not null && settings.PetTop is not null)
        {
            Left = settings.PetLeft.Value;
            Top = settings.PetTop.Value;
        }

        LoadCharacter(paths, settings.CurrentCharacterId, validator);
        Sprite.Completed += OnSpriteAnimationCompleted;
        _eventBus.Subscribe<ActionRequested>(OnActionRequested);
        SourceInitialized += (_, _) => MonitorPlacement.ClampAndDetectEdge(this);
        Loaded += (_, _) =>
        {
            StartRestAnimation();
            ScheduleNextIdleAction();
        };
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
            Sprite.Frames = string.Join(",", IdleFrames);
            Sprite.Fps = 2;

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

    private void StartRestAnimation()
    {
        if (_isDragging || Sprite.Visibility != Visibility.Visible)
        {
            return;
        }

        if (_sharinganState == 0)
        {
            Sprite.Play(IdleFrames, 2);
        }
        else
        {
            Sprite.Play([5 + _sharinganState], 2);
        }
    }

    private void PlayOnce(IEnumerable<int> frames, int fps)
    {
        if (!_isDragging && Sprite.Visibility == Visibility.Visible)
        {
            Sprite.PlayOnce(frames, fps);
        }
    }

    private void OnSpriteAnimationCompleted(object? sender, EventArgs e) => StartRestAnimation();

    private void ScheduleNextIdleAction()
    {
        _idleActionTimer.Stop();
        _idleActionTimer.Interval = TimeSpan.FromSeconds(Random.Shared.Next(15, 31));
        _idleActionTimer.Start();
    }

    private void OnIdleActionTimerTick(object? sender, EventArgs e)
    {
        ScheduleNextIdleAction();
        if (_settings.IdleActionsEnabled && !_settings.ReduceMotion &&
            !_isDragging && _sharinganState == 0)
        {
            PlayOnce([Random.Shared.Next(4, 8)], 4);
        }
    }

    private void OnActionRequested(ActionRequested message)
    {
        Dispatcher.Invoke(() =>
        {
            int[] frames = message.ActionId switch
            {
                "clicked" => ReactionFrames,
                "celebrate" => [7, 7, 3],
                var action when action.StartsWith("reminder.", StringComparison.Ordinal) => [6, 6, 3],
                "focus" => [1, 2],
                "relax" => [1, 3],
                _ => IdleFrames
            };
            PlayOnce(frames, message.ActionId == "clicked" ? 8 : 6);
        });
    }

    private void OnMouseEnter(object sender, MouseEventArgs e)
    {
        if (_isDragging || _settings.ReduceMotion)
        {
            return;
        }

        var direction = e.GetPosition(this).X < ActualWidth / 2 ? 15d : -15d;
        _hoverStoryboard?.Stop(PetVisual);
        HoverTranslate.X = 0;

        var nudge = new DoubleAnimation
        {
            From = 0,
            To = direction,
            Duration = TimeSpan.FromMilliseconds(250),
            AutoReverse = true,
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        Storyboard.SetTarget(nudge, HoverTranslate);
        Storyboard.SetTargetProperty(nudge, new PropertyPath(TranslateTransform.XProperty));
        _hoverStoryboard = new Storyboard();
        _hoverStoryboard.Children.Add(nudge);
        _hoverStoryboard.Begin(PetVisual, HandoffBehavior.SnapshotAndReplace, isControllable: true);
        PlayOnce([4, 5, 4, 5], 8);
    }

    private void OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left || _isDragging)
        {
            return;
        }

        _suppressNextClick = true;
        _sharinganState = (_sharinganState + 1) % 3;
        StartRestAnimation();
        e.Handled = true;
    }

    private void OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left)
        {
            return;
        }

        _mouseDown = e.GetPosition(this);
        _dragOccurred = false;
    }

    private void OnMouseMove(object sender, MouseEventArgs e)
    {
        if (_isDragging || e.LeftButton != MouseButtonState.Pressed)
        {
            return;
        }

        var current = e.GetPosition(this);
        var delta = current - _mouseDown;
        if (Math.Abs(delta.X) < SystemParameters.MinimumHorizontalDragDistance &&
            Math.Abs(delta.Y) < SystemParameters.MinimumVerticalDragDistance)
        {
            return;
        }

        _dragOccurred = true;
        _isDragging = true;
        _idleActionTimer.Stop();
        _hoverStoryboard?.Stop(PetVisual);
        HoverTranslate.X = 0;
        Sprite.Pause();
        try
        {
            DragMove();
        }
        catch (InvalidOperationException)
        {
            // Mouse was released before WPF entered the native move loop.
        }
        finally
        {
            _isDragging = false;
            StartRestAnimation();
            ScheduleNextIdleAction();
            SavePlacementAfterDrag();
        }
    }

    private async void OnMouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left || _dragOccurred)
        {
            return;
        }

        if (_suppressNextClick)
        {
            _suppressNextClick = false;
            return;
        }

        if ((e.GetPosition(this) - _mouseDown).Length < 6)
        {
            _eventBus.Publish(new ActionRequested("clicked", DateTimeOffset.Now));
        }

        await SavePlacementAsync();
    }

    private async void SavePlacementAfterDrag() => await SavePlacementAsync();

    private async Task SavePlacementAsync()
    {
        _settings = _settings with { PetLeft = Left, PetTop = Top };
        var edge = MonitorPlacement.ClampAndDetectEdge(this);
        if (edge is not null && _settings.EdgeActionsEnabled)
        {
            _eventBus.Publish(new ActionRequested(edge, DateTimeOffset.Now));
        }

        _settings = _settings with { PetLeft = Left, PetTop = Top };
        await _configStore.SaveAsync(_settings);
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

        _idleActionTimer.Stop();
        Sprite.Completed -= OnSpriteAnimationCompleted;
        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
    }

    private void OnDisplaySettingsChanged(object? sender, EventArgs e)
    {
        Dispatcher.Invoke(() => MonitorPlacement.ClampAndDetectEdge(this));
    }
}
