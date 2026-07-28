using Companion.Domain.Entities;
using Companion.Infrastructure.Storage;
using Microsoft.Data.Sqlite;

namespace Companion.IntegrationTests;

public sealed class SqliteCompanionStoreTests : IDisposable
{
    private readonly string _root = Path.Combine(
        Path.GetTempPath(),
        $"offline-companion-tests-{Guid.NewGuid():N}");

    [Fact]
    public async Task TodoRoundTripPersistsAllCoreFields()
    {
        Directory.CreateDirectory(_root);
        var connection = new SqliteConnectionStringBuilder
        {
            DataSource = Path.Combine(_root, "companion.db")
        }.ToString();
        var store = new SqliteCompanionStore(connection);
        await store.InitializeAsync();
        var now = DateTimeOffset.UtcNow;
        var todo = new TodoItem(
            Guid.NewGuid(),
            "测试待办",
            "仅本地",
            TodoPriority.High,
            now.AddHours(1),
            now.AddMinutes(30),
            null,
            now,
            now);

        await store.UpsertTodoAsync(todo);
        var loaded = await store.GetTodosAsync(includeCompleted: false);

        var item = Assert.Single(loaded);
        Assert.Equal(todo.Id, item.Id);
        Assert.Equal(todo.Title, item.Title);
        Assert.Equal(todo.Priority, item.Priority);
    }

    public void Dispose()
    {
        if (Directory.Exists(_root))
        {
            Directory.Delete(_root, recursive: true);
        }
    }
}
