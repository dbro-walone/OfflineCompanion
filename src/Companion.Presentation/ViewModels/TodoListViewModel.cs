using System.Collections.ObjectModel;
using Companion.Application.Services;
using Companion.Domain.Entities;
using Companion.Presentation.Mvvm;

namespace Companion.Presentation.ViewModels;

public sealed class TodoListViewModel : ObservableObject
{
    private readonly TodoService _service;
    private string _newTitle = string.Empty;
    private bool _includeCompleted;

    public TodoListViewModel(TodoService service)
    {
        _service = service;
        AddCommand = new AsyncRelayCommand(AddAsync, () => !string.IsNullOrWhiteSpace(NewTitle));
        RefreshCommand = new AsyncRelayCommand(RefreshAsync);
    }

    public ObservableCollection<TodoItem> Items { get; } = [];
    public AsyncRelayCommand AddCommand { get; }
    public AsyncRelayCommand RefreshCommand { get; }

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

    public async Task RefreshAsync()
    {
        var items = await _service.ListAsync(IncludeCompleted);
        Items.Clear();
        foreach (var item in items)
        {
            Items.Add(item);
        }
    }

    public async Task ToggleAsync(TodoItem item)
    {
        await _service.SetCompletedAsync(item, !item.IsCompleted);
        await RefreshAsync();
    }

    private async Task AddAsync()
    {
        await _service.CreateAsync(NewTitle);
        NewTitle = string.Empty;
        await RefreshAsync();
    }
}
