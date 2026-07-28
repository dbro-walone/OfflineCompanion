using System.Windows;
using System.Windows.Threading;
using Companion.Application.Services;
using Companion.Domain.Entities;

namespace Companion.App;

public partial class TimerWindow
{
    private readonly PomodoroService _service;
    private readonly TodoService _todoService;
    private readonly DispatcherTimer _timer;
    private PomodoroSession? _session;
    private TodoItem? _activeTodo;
    private Guid? _linkedSessionId;
    private bool _completionRecorded;

    public TimerWindow(PomodoroService service, TodoService todoService)
    {
        InitializeComponent();
        _service = service;
        _todoService = todoService;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += async (_, _) => await RefreshAsync();
        Loaded += async (_, _) =>
        {
            if (_activeTodo is not null)
            {
                await StartLinkedFocusAsync();
            }
            else
            {
                _session = await _service.RestoreAsync();
            }
            UpdateDisplay();
            _timer.Start();
        };
        Closed += (_, _) => _timer.Stop();
    }

    public void FocusOn(TodoItem item)
    {
        _activeTodo = item;
        ActiveTodoText.Text = $"正在专注：{item.Title}";
        ActiveTodoText.Visibility = Visibility.Visible;
    }

    private async void StartTimer(object sender, RoutedEventArgs e)
    {
        _session = await _service.StartAsync();
        _linkedSessionId = _activeTodo is null ? null : _session.Id;
        _completionRecorded = false;
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
        await RecordLinkedCompletionAsync();
        UpdateDisplay();
    }

    private async Task StartLinkedFocusAsync()
    {
        _session = await _service.StartAsync(PomodoroPhase.Focus);
        _linkedSessionId = _session.Id;
        _completionRecorded = false;
    }

    private async Task RecordLinkedCompletionAsync()
    {
        if (_completionRecorded ||
            _activeTodo is null ||
            _session is not
            {
                Status: PomodoroStatus.Completed,
                Phase: PomodoroPhase.Focus
            } ||
            _session.Id != _linkedSessionId)
        {
            return;
        }

        _activeTodo = await _todoService.CompletePomodoroAsync(_activeTodo);
        _completionRecorded = true;
        ActiveTodoText.Text =
            $"已完成：{_activeTodo.Title}（{_activeTodo.CompletedPomodoros}/{_activeTodo.EstimatedPomodoros}）";
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
