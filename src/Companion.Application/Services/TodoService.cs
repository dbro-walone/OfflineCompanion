using Companion.Application.Abstractions;
using Companion.Application.Events;
using Companion.Domain.Entities;

namespace Companion.Application.Services;

public sealed class TodoService(ICompanionStore store, IClock clock, IEventBus eventBus)
{
    public Task<IReadOnlyList<TodoItem>> ListAsync(
        bool includeCompleted = false,
        CancellationToken cancellationToken = default) =>
        store.GetTodosAsync(includeCompleted, cancellationToken);

    public async Task<TodoItem> CreateAsync(
        string title,
        string? note = null,
        TodoPriority priority = TodoPriority.Normal,
        DateTimeOffset? dueAt = null,
        DateTimeOffset? reminderAt = null,
        int estimatedPomodoros = 1,
        DateTimeOffset? dueTime = null,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(title))
        {
            throw new ArgumentException("待办标题不能为空。", nameof(title));
        }
        if (estimatedPomodoros is < 1 or > 10)
        {
            throw new ArgumentOutOfRangeException(
                nameof(estimatedPomodoros),
                "预计番茄数必须在 1 到 10 之间。");
        }

        var now = clock.Now;
        var item = new TodoItem(
            Guid.NewGuid(),
            title.Trim(),
            string.IsNullOrWhiteSpace(note) ? null : note.Trim(),
            priority,
            dueAt,
            reminderAt,
            null,
            now,
            now,
            estimatedPomodoros,
            0,
            dueTime);
        await store.UpsertTodoAsync(item, cancellationToken);
        return item;
    }

    public async Task<TodoItem> SetCompletedAsync(
        TodoItem item,
        bool completed,
        CancellationToken cancellationToken = default)
    {
        var updated = completed ? item.Complete(clock.Now) : item.Restore(clock.Now);
        await store.UpsertTodoAsync(updated, cancellationToken);
        if (completed)
        {
            eventBus.Publish(new ActionRequested("celebrate", clock.Now));
        }

        return updated;
    }

    public Task DeleteAsync(Guid id, CancellationToken cancellationToken = default) =>
        store.DeleteTodoAsync(id, cancellationToken);

    public Task ClearCompletedAsync(CancellationToken cancellationToken = default) =>
        store.DeleteCompletedTodosAsync(cancellationToken);

    public async Task<TodoItem> CompletePomodoroAsync(
        TodoItem item,
        CancellationToken cancellationToken = default)
    {
        var latest = (await store.GetTodosAsync(true, cancellationToken))
            .FirstOrDefault(candidate => candidate.Id == item.Id) ?? item;
        var updated = latest.CompletePomodoro(clock.Now);
        await store.UpsertTodoAsync(updated, cancellationToken);
        return updated;
    }
}
