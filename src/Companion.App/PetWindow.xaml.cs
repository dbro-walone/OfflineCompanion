using System.ComponentModel;
using System.Windows;
using System.Windows.Input;
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
    private const double BaseWidth = 250;
    private const double BaseHeight = 330;
    private static readonly int[] IdleFrames = [0, 1, 2, 3];
    private static readonly int[] ReactionFrames = [4, 5];

    private readonly IServiceProvider _services;
    private readonly JsonConfigStore _configStore;
    private readonly IEventBus _eventBus;
    private readonly DispatcherTimer _idleActionTimer;
    private readonly IDisposable _settingsChangedSubscription;
    private AppSettings _settings;
    private Point _mouseDown;
    private bool _allowClose;
    private bool _dragOccurred;
    private bool _isDragging;
    private bool _suppressNextClick;
    private int _sharinganState;
    private int _currentIdlePose;
    private int _crossfadeVersion;

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
        Width = BaseWidth * settings.PetScale;
        Height = BaseHeight * settings.PetScale;
        if (settings.PetLeft is not null && settings.PetTop is not null)
        {
            Left = settings.PetLeft.Value;
            Top = settings.PetTop.Value;
        }

        LoadCharacter(paths, settings.CurrentCharacterId, validator);
        Sprite.Completed += OnSpriteAnimationCompleted;
        _eventBus.Subscribe<ActionRequested>(OnActionRequested);
        _settingsChangedSubscription = _eventBus.Subscribe<SettingsChanged>(OnSettingsChanged);
        SourceInitialized += (_, _) => MonitorPlacement.ClampAndDetectEdge(this);
        Loaded += (_, _) =>
        {
            StartRestAnimation();
            RestartIdleActionTimer();
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
            CrossfadeToFrame(_currentIdlePose);
        }
        else
        {
            CrossfadeToFrame(5 + _sharinganState);
        }
    }

    private void CrossfadeToFrame(int frame)
    {
        if (Sprite.Visibility != Visibility.Visible)
        {
            return;
        }

        var version = ++_crossfadeVersion;
        var fadeOut = new DoubleAnimation
        {
            To = 0,
            Duration = TimeSpan.FromMilliseconds(200),
            FillBehavior = FillBehavior.HoldEnd
        };
        fadeOut.Completed += (_, _) =>
        {
            if (version != _crossfadeVersion)
            {
                return;
            }

            Sprite.ShowStaticFrame(frame);
            Sprite.Opacity = 1;
            var fadeIn = new DoubleAnimation
            {
                From = 0,
                To = 1,
                Duration = TimeSpan.FromMilliseconds(200),
                FillBehavior = FillBehavior.Stop
            };
            Sprite.BeginAnimation(OpacityProperty, fadeIn, HandoffBehavior.SnapshotAndReplace);
        };
        Sprite.BeginAnimation(OpacityProperty, fadeOut, HandoffBehavior.SnapshotAndReplace);
    }

    private void PlayOnce(IEnumerable<int> frames, int fps)
    {
        if (!_isDragging && Sprite.Visibility == Visibility.Visible)
        {
            _crossfadeVersion++;
            Sprite.BeginAnimation(OpacityProperty, null);
            Sprite.Opacity = 1;
            Sprite.PlayOnce(frames, fps);
        }
    }

    private void OnSpriteAnimationCompleted(object? sender, EventArgs e)
    {
        // After a one-shot animation finishes, return to static pose — no looping.
        if (_sharinganState > 0)
        {
            Sprite.ShowStaticFrame(5 + _sharinganState);
        }
        else
        {
            Sprite.ShowStaticFrame(_currentIdlePose);
        }
    }

    private void ScheduleNextIdleAction()
    {
        _idleActionTimer.Stop();
        _idleActionTimer.Interval = TimeSpan.FromSeconds(Random.Shared.Next(15, 31));
        _idleActionTimer.Start();
    }

    private void RestartIdleActionTimer()
    {
        _idleActionTimer.Stop();
        if (_settings.IdleActionsEnabled && !_settings.ReduceMotion)
        {
            ScheduleNextIdleAction();
        }
    }

    private void OnIdleActionTimerTick(object? sender, EventArgs e)
    {
        ScheduleNextIdleAction();
        if (_settings.IdleActionsEnabled && !_settings.ReduceMotion &&
            !_isDragging && _sharinganState == 0)
        {
            // Cycle through poses: 0 (standing) → 1 → 2 → 3 → 0 ...
            // Each pose is held static for 15-30 seconds, then switches.
            // Like the Rising antivirus lion — no animation loop, just pose changes.
            _currentIdlePose = (_currentIdlePose + 1) % 4;
            CrossfadeToFrame(_currentIdlePose);
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

    private void OnSettingsChanged(SettingsChanged message)
    {
        Dispatcher.Invoke(() =>
        {
            _settings = message.Settings;
            Width = BaseWidth * _settings.PetScale;
            Height = BaseHeight * _settings.PetScale;
            Topmost = _settings.Topmost;
            RestartIdleActionTimer();
        });
    }

    private void OnMouseEnter(object sender, MouseEventArgs e)
    {
        if (_isDragging || _settings.ReduceMotion)
        {
            return;
        }

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
            RestartIdleActionTimer();
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
        _settingsChangedSubscription.Dispose();
        Sprite.Completed -= OnSpriteAnimationCompleted;
        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
    }

    private void OnDisplaySettingsChanged(object? sender, EventArgs e)
    {
        Dispatcher.Invoke(() => MonitorPlacement.ClampAndDetectEdge(this));
    }
}
