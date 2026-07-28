using Companion.Domain.Entities;

namespace Companion.Application.Abstractions;

public interface ICompanionStore
{
    Task InitializeAsync(CancellationToken cancellationToken = default);
    Task<IReadOnlyList<TodoItem>> GetTodosAsync(bool includeCompleted, CancellationToken cancellationToken = default);
    Task UpsertTodoAsync(TodoItem item, CancellationToken cancellationToken = default);
    Task DeleteTodoAsync(Guid id, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<Reminder>> GetActiveRemindersAsync(CancellationToken cancellationToken = default);
    Task UpsertReminderAsync(Reminder reminder, CancellationToken cancellationToken = default);
    Task<PomodoroSession?> GetCurrentPomodoroAsync(CancellationToken cancellationToken = default);
    Task UpsertPomodoroAsync(PomodoroSession session, CancellationToken cancellationToken = default);
}
