using System.Windows;
using System.Windows.Threading;
using Companion.Application.Services;
using Companion.Domain.Entities;

namespace Companion.App;

public partial class TimerWindow
{
    private readonly PomodoroService _service;
    private readonly DispatcherTimer _timer;
    private PomodoroSession? _session;

    public TimerWindow(PomodoroService service)
    {
        InitializeComponent();
        _service = service;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += async (_, _) => await RefreshAsync();
        Loaded += async (_, _) =>
        {
            _session = await _service.RestoreAsync();
            UpdateDisplay();
            _timer.Start();
        };
        Closed += (_, _) => _timer.Stop();
    }

    private async void StartTimer(object sender, RoutedEventArgs e)
    {
        _session = await _service.StartAsync();
        UpdateDisplay();
    }

    private async void TogglePause(object sender, RoutedEventArgs e)
    {
        if (_session is null)
        {
            return;
        }

        _session = _session.Status == PomodoroStatus.Paused
            ? await _service.ResumeAsync(_session)
            : await _service.PauseAsync(_session);
        UpdateDisplay();
    }

    private async Task RefreshAsync()
    {
        _session = await _service.RestoreAsync();
        UpdateDisplay();
    }

    private void UpdateDisplay()
    {
        if (_session is null)
        {
            TimeText.Text = "25:00";
            return;
        }

        var remaining = _session.Status == PomodoroStatus.Running
            ? Math.Max(0, (int)(_session.ExpectedEndAt - DateTimeOffset.Now).TotalSeconds)
            : _session.RemainingSeconds;
        TimeText.Text = TimeSpan.FromSeconds(remaining).ToString(@"mm\:ss");
    }
}
