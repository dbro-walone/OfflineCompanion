using System.Collections.ObjectModel;
using Companion.Application.Services;
using Companion.Domain.Entities;
using Companion.Presentation.Mvvm;

namespace Companion.Presentation.ViewModels;

public sealed class TodoListViewModel : ObservableObject
{
    private readonly TodoService _service;
    private readonly List<TodoItem> _allItems = [];
    private string _newTitle = string.Empty;
    private bool _includeCompleted;
    private int _newEstimatedPomodoros = 1;
    private DateTime? _newDueDate;
    private string _newDueTime = string.Empty;
    private string _validationMessage = string.Empty;
    private int _currentPage = 1;

    private const int PageSize = 10;

    public TodoListViewModel(TodoService service)
    {
        _service = service;
        AddCommand = new AsyncRelayCommand(AddAsync, () => !string.IsNullOrWhiteSpace(NewTitle));
        RefreshCommand = new AsyncRelayCommand(RefreshAsync);
    }

    public ObservableCollection<TodoItem> Items { get; } = [];
    public AsyncRelayCommand AddCommand { get; }
    public AsyncRelayCommand RefreshCommand { get; }
    public IReadOnlyList<int> PomodoroOptions { get; } = Enumerable.Range(1, 10).ToArray();

    public string NewTitle
    {
        get => _newTitle;
        set
        {
            if (SetProperty(ref _newTitle, value))
            {
                AddCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool IncludeCompleted
    {
        get => _includeCompleted;
        set
        {
            if (SetProperty(ref _includeCompleted, value))
            {
                _ = RefreshAsync();
            }
        }
    }

    public int NewEstimatedPomodoros
    {
        get => _newEstimatedPomodoros;
        set => SetProperty(ref _newEstimatedPomodoros, Math.Clamp(value, 1, 10));
    }

    public DateTime? NewDueDate
    {
        get => _newDueDate;
        set => SetProperty(ref _newDueDate, value);
    }

    public string NewDueTime
    {
        get => _newDueTime;
        set => SetProperty(ref _newDueTime, value);
    }

    public string ValidationMessage
    {
        get => _validationMessage;
        private set => SetProperty(ref _validationMessage, value);
    }

    public int CurrentPage
    {
        get => _currentPage;
        private set
        {
            if (SetProperty(ref _currentPage, value))
            {
                RaisePropertyChanged(nameof(PageStatus));
                RaisePropertyChanged(nameof(CanGoPrevious));
                RaisePropertyChanged(nameof(CanGoNext));
            }
        }
    }

    public int TotalPages => Math.Max(1, (int)Math.Ceiling(_allItems.Count / (double)PageSize));
    public string PageStatus => $"第 {CurrentPage} / {TotalPages} 页";
    public bool CanGoPrevious => CurrentPage > 1;
    public bool CanGoNext => CurrentPage < TotalPages;

    public async Task RefreshAsync()
    {
        var items = await _service.ListAsync(IncludeCompleted);
        _allItems.Clear();
        _allItems.AddRange(items);
        CurrentPage = Math.Min(CurrentPage, TotalPages);
        ShowCurrentPage();
    }

    public async Task ToggleAsync(TodoItem item)
    {
        await _service.SetCompletedAsync(item, !item.IsCompleted);
        await RefreshAsync();
    }

    public void PreviousPage()
    {
        if (CanGoPrevious)
        {
            CurrentPage--;
            ShowCurrentPage();
        }
    }

    public void NextPage()
    {
        if (CanGoNext)
        {
            CurrentPage++;
            ShowCurrentPage();
        }
    }

    public async Task ClearCompletedAsync()
    {
        await _service.ClearCompletedAsync();
        await RefreshAsync();
    }

    private async Task AddAsync()
    {
        DateTimeOffset? dueTime;
        try
        {
            dueTime = ParseDueTime();
            ValidationMessage = string.Empty;
        }
        catch (FormatException ex)
        {
            ValidationMessage = ex.Message;
            return;
        }

        await _service.CreateAsync(
            NewTitle,
            dueAt: dueTime,
            estimatedPomodoros: NewEstimatedPomodoros,
            dueTime: dueTime);
        NewTitle = string.Empty;
        NewEstimatedPomodoros = 1;
        NewDueDate = null;
        NewDueTime = string.Empty;
        CurrentPage = 1;
        await RefreshAsync();
    }

    private DateTimeOffset? ParseDueTime()
    {
        if (NewDueDate is null)
        {
            return null;
        }

        var time = TimeSpan.Zero;
        if (!string.IsNullOrWhiteSpace(NewDueTime) &&
            !TimeSpan.TryParse(NewDueTime.Trim(), out time))
        {
            throw new FormatException("截止时间格式应为 HH:mm，例如 18:30。");
        }

        var local = NewDueDate.Value.Date.Add(time);
        return new DateTimeOffset(local, TimeZoneInfo.Local.GetUtcOffset(local));
    }

    private void ShowCurrentPage()
    {
        Items.Clear();
        foreach (var item in _allItems
                     .Skip((CurrentPage - 1) * PageSize)
                     .Take(PageSize))
        {
            Items.Add(item);
        }

        RaisePropertyChanged(nameof(TotalPages));
        RaisePropertyChanged(nameof(PageStatus));
        RaisePropertyChanged(nameof(CanGoPrevious));
        RaisePropertyChanged(nameof(CanGoNext));
    }
}
