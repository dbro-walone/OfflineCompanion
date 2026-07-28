using System.Globalization;
using System.Text.Json;
using Companion.Application.Abstractions;
using Companion.Domain.Entities;
using Microsoft.Data.Sqlite;

namespace Companion.Infrastructure.Storage;

public sealed class SqliteCompanionStore(string connectionString) : ICompanionStore
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        var builder = new SqliteConnectionStringBuilder(connectionString);
        if (!string.IsNullOrWhiteSpace(builder.DataSource))
        {
            Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(builder.DataSource))!);
        }

        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await ExecuteAsync(connection, "PRAGMA journal_mode=WAL;", cancellationToken);
        await ExecuteAsync(connection, "PRAGMA foreign_keys=ON;", cancellationToken);
        await ExecuteAsync(connection, """
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );
            INSERT INTO schema_version(version)
            SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_version);

            CREATE TABLE IF NOT EXISTS todo_items (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                note TEXT NULL,
                priority INTEGER NOT NULL,
                due_at TEXT NULL,
                reminder_at TEXT NULL,
                completed_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                estimated_pomodoros INTEGER NOT NULL DEFAULT 1,
                completed_pomodoros INTEGER NOT NULL DEFAULT 0,
                due_time TEXT NULL
            );
            CREATE INDEX IF NOT EXISTS ix_todo_due ON todo_items(due_at);

            CREATE TABLE IF NOT EXISTS reminders (
                id TEXT PRIMARY KEY,
                todo_id TEXT NULL,
                title TEXT NOT NULL,
                schedule_type INTEGER NOT NULL,
                local_time TEXT NOT NULL,
                weekdays TEXT NOT NULL,
                start_date TEXT NULL,
                end_date TEXT NULL,
                next_trigger_at TEXT NOT NULL,
                status INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(todo_id) REFERENCES todo_items(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS ix_reminder_next ON reminders(status, next_trigger_at);

            CREATE TABLE IF NOT EXISTS pomodoro_sessions (
                id TEXT PRIMARY KEY,
                phase INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                expected_end_at TEXT NOT NULL,
                paused_at TEXT NULL,
                remaining_seconds INTEGER NOT NULL,
                completed_focus_rounds INTEGER NOT NULL,
                status INTEGER NOT NULL
            );
            """, cancellationToken);

        await EnsureColumnAsync(
            connection,
            "todo_items",
            "estimated_pomodoros",
            "INTEGER NOT NULL DEFAULT 1",
            cancellationToken);
        await EnsureColumnAsync(
            connection,
            "todo_items",
            "completed_pomodoros",
            "INTEGER NOT NULL DEFAULT 0",
            cancellationToken);
        await EnsureColumnAsync(
            connection,
            "todo_items",
            "due_time",
            "TEXT NULL",
            cancellationToken);
    }

    public async Task<IReadOnlyList<TodoItem>> GetTodosAsync(
        bool includeCompleted,
        CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT id, title, note, priority, due_at, reminder_at, completed_at, created_at, updated_at,
                   estimated_pomodoros, completed_pomodoros, due_time
            FROM todo_items
            WHERE $includeCompleted = 1 OR completed_at IS NULL
            ORDER BY completed_at IS NOT NULL, due_at IS NULL, due_at, priority DESC, created_at DESC;
            """;
        command.Parameters.AddWithValue("$includeCompleted", includeCompleted ? 1 : 0);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var result = new List<TodoItem>();
        while (await reader.ReadAsync(cancellationToken))
        {
            result.Add(new TodoItem(
                Guid.Parse(reader.GetString(0)),
                reader.GetString(1),
                reader.IsDBNull(2) ? null : reader.GetString(2),
                (TodoPriority)reader.GetInt32(3),
                ParseNullableDate(reader, 4),
                ParseNullableDate(reader, 5),
                ParseNullableDate(reader, 6),
                DateTimeOffset.Parse(reader.GetString(7), CultureInfo.InvariantCulture),
                DateTimeOffset.Parse(reader.GetString(8), CultureInfo.InvariantCulture),
                reader.GetInt32(9),
                reader.GetInt32(10),
                ParseNullableDate(reader, 11)));
        }

        return result;
    }

    public async Task UpsertTodoAsync(TodoItem item, CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            INSERT INTO todo_items
                (id, title, note, priority, due_at, reminder_at, completed_at, created_at, updated_at,
                 estimated_pomodoros, completed_pomodoros, due_time)
            VALUES
                ($id, $title, $note, $priority, $dueAt, $reminderAt, $completedAt, $createdAt, $updatedAt,
                 $estimatedPomodoros, $completedPomodoros, $dueTime)
            ON CONFLICT(id) DO UPDATE SET
                title=excluded.title,
                note=excluded.note,
                priority=excluded.priority,
                due_at=excluded.due_at,
                reminder_at=excluded.reminder_at,
                completed_at=excluded.completed_at,
                estimated_pomodoros=excluded.estimated_pomodoros,
                completed_pomodoros=excluded.completed_pomodoros,
                due_time=excluded.due_time,
                updated_at=excluded.updated_at;
            """;
        command.Parameters.AddWithValue("$id", item.Id.ToString());
        command.Parameters.AddWithValue("$title", item.Title);
        command.Parameters.AddWithValue("$note", Db(item.Note));
        command.Parameters.AddWithValue("$priority", (int)item.Priority);
        command.Parameters.AddWithValue("$dueAt", Db(item.DueAt));
        command.Parameters.AddWithValue("$reminderAt", Db(item.ReminderAt));
        command.Parameters.AddWithValue("$completedAt", Db(item.CompletedAt));
        command.Parameters.AddWithValue("$createdAt", item.CreatedAt.ToString("O"));
        command.Parameters.AddWithValue("$updatedAt", item.UpdatedAt.ToString("O"));
        command.Parameters.AddWithValue("$estimatedPomodoros", item.EstimatedPomodoros);
        command.Parameters.AddWithValue("$completedPomodoros", item.CompletedPomodoros);
        command.Parameters.AddWithValue("$dueTime", Db(item.DueTime));
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task DeleteTodoAsync(Guid id, CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "DELETE FROM todo_items WHERE id=$id;";
        command.Parameters.AddWithValue("$id", id.ToString());
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task DeleteCompletedTodosAsync(CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = "DELETE FROM todo_items WHERE completed_at IS NOT NULL;";
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task<IReadOnlyList<Reminder>> GetActiveRemindersAsync(
        CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT id, todo_id, title, schedule_type, local_time, weekdays, start_date, end_date,
                   next_trigger_at, status, created_at
            FROM reminders
            WHERE status=$active
            ORDER BY next_trigger_at;
            """;
        command.Parameters.AddWithValue("$active", (int)ReminderStatus.Active);
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var result = new List<Reminder>();
        while (await reader.ReadAsync(cancellationToken))
        {
            result.Add(new Reminder(
                Guid.Parse(reader.GetString(0)),
                reader.IsDBNull(1) ? null : Guid.Parse(reader.GetString(1)),
                reader.GetString(2),
                (ReminderScheduleType)reader.GetInt32(3),
                TimeOnly.Parse(reader.GetString(4), CultureInfo.InvariantCulture),
                JsonSerializer.Deserialize<DayOfWeek[]>(reader.GetString(5), JsonOptions) ?? [],
                ParseNullableDateOnly(reader, 6),
                ParseNullableDateOnly(reader, 7),
                DateTimeOffset.Parse(reader.GetString(8), CultureInfo.InvariantCulture),
                (ReminderStatus)reader.GetInt32(9),
                DateTimeOffset.Parse(reader.GetString(10), CultureInfo.InvariantCulture)));
        }

        return result;
    }

    public async Task UpsertReminderAsync(Reminder reminder, CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            INSERT INTO reminders
                (id, todo_id, title, schedule_type, local_time, weekdays, start_date, end_date,
                 next_trigger_at, status, created_at)
            VALUES
                ($id, $todoId, $title, $scheduleType, $localTime, $weekdays, $startDate, $endDate,
                 $nextTriggerAt, $status, $createdAt)
            ON CONFLICT(id) DO UPDATE SET
                todo_id=excluded.todo_id,
                title=excluded.title,
                schedule_type=excluded.schedule_type,
                local_time=excluded.local_time,
                weekdays=excluded.weekdays,
                start_date=excluded.start_date,
                end_date=excluded.end_date,
                next_trigger_at=excluded.next_trigger_at,
                status=excluded.status;
            """;
        command.Parameters.AddWithValue("$id", reminder.Id.ToString());
        command.Parameters.AddWithValue("$todoId", Db(reminder.TodoId?.ToString()));
        command.Parameters.AddWithValue("$title", reminder.Title);
        command.Parameters.AddWithValue("$scheduleType", (int)reminder.ScheduleType);
        command.Parameters.AddWithValue("$localTime", reminder.LocalTime.ToString("HH:mm:ss"));
        command.Parameters.AddWithValue("$weekdays", JsonSerializer.Serialize(reminder.Weekdays, JsonOptions));
        command.Parameters.AddWithValue("$startDate", Db(reminder.StartDate?.ToString("O")));
        command.Parameters.AddWithValue("$endDate", Db(reminder.EndDate?.ToString("O")));
        command.Parameters.AddWithValue("$nextTriggerAt", reminder.NextTriggerAt.ToString("O"));
        command.Parameters.AddWithValue("$status", (int)reminder.Status);
        command.Parameters.AddWithValue("$createdAt", reminder.CreatedAt.ToString("O"));
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    public async Task<PomodoroSession?> GetCurrentPomodoroAsync(
        CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT id, phase, started_at, expected_end_at, paused_at, remaining_seconds,
                   completed_focus_rounds, status
            FROM pomodoro_sessions
            ORDER BY started_at DESC
            LIMIT 1;
            """;
        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        if (!await reader.ReadAsync(cancellationToken))
        {
            return null;
        }

        return new PomodoroSession(
            Guid.Parse(reader.GetString(0)),
            (PomodoroPhase)reader.GetInt32(1),
            DateTimeOffset.Parse(reader.GetString(2), CultureInfo.InvariantCulture),
            DateTimeOffset.Parse(reader.GetString(3), CultureInfo.InvariantCulture),
            ParseNullableDate(reader, 4),
            reader.GetInt32(5),
            reader.GetInt32(6),
            (PomodoroStatus)reader.GetInt32(7));
    }

    public async Task UpsertPomodoroAsync(
        PomodoroSession session,
        CancellationToken cancellationToken = default)
    {
        await using var connection = new SqliteConnection(connectionString);
        await connection.OpenAsync(cancellationToken);
        await using var command = connection.CreateCommand();
        command.CommandText = """
            INSERT INTO pomodoro_sessions
                (id, phase, started_at, expected_end_at, paused_at, remaining_seconds,
                 completed_focus_rounds, status)
            VALUES
                ($id, $phase, $startedAt, $expectedEndAt, $pausedAt, $remaining,
                 $completedRounds, $status)
            ON CONFLICT(id) DO UPDATE SET
                phase=excluded.phase,
                expected_end_at=excluded.expected_end_at,
                paused_at=excluded.paused_at,
                remaining_seconds=excluded.remaining_seconds,
                completed_focus_rounds=excluded.completed_focus_rounds,
                status=excluded.status;
            """;
        command.Parameters.AddWithValue("$id", session.Id.ToString());
        command.Parameters.AddWithValue("$phase", (int)session.Phase);
        command.Parameters.AddWithValue("$startedAt", session.StartedAt.ToString("O"));
        command.Parameters.AddWithValue("$expectedEndAt", session.ExpectedEndAt.ToString("O"));
        command.Parameters.AddWithValue("$pausedAt", Db(session.PausedAt));
        command.Parameters.AddWithValue("$remaining", session.RemainingSeconds);
        command.Parameters.AddWithValue("$completedRounds", session.CompletedFocusRounds);
        command.Parameters.AddWithValue("$status", (int)session.Status);
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    private static async Task ExecuteAsync(
        SqliteConnection connection,
        string sql,
        CancellationToken cancellationToken)
    {
        await using var command = connection.CreateCommand();
        command.CommandText = sql;
        await command.ExecuteNonQueryAsync(cancellationToken);
    }

    private static async Task EnsureColumnAsync(
        SqliteConnection connection,
        string table,
        string column,
        string definition,
        CancellationToken cancellationToken)
    {
        await using var inspect = connection.CreateCommand();
        inspect.CommandText = $"PRAGMA table_info({table});";
        await using var reader = await inspect.ExecuteReaderAsync(cancellationToken);
        while (await reader.ReadAsync(cancellationToken))
        {
            if (string.Equals(reader.GetString(1), column, StringComparison.OrdinalIgnoreCase))
            {
                return;
            }
        }

        await reader.DisposeAsync();
        await ExecuteAsync(
            connection,
            $"ALTER TABLE {table} ADD COLUMN {column} {definition};",
            cancellationToken);
    }

    private static object Db(string? value) => value is null ? DBNull.Value : value;
    private static object Db(DateTimeOffset? value) => value is null ? DBNull.Value : value.Value.ToString("O");

    private static DateTimeOffset? ParseNullableDate(SqliteDataReader reader, int ordinal) =>
        reader.IsDBNull(ordinal)
            ? null
            : DateTimeOffset.Parse(reader.GetString(ordinal), CultureInfo.InvariantCulture);

    private static DateOnly? ParseNullableDateOnly(SqliteDataReader reader, int ordinal) =>
        reader.IsDBNull(ordinal)
            ? null
            : DateOnly.Parse(reader.GetString(ordinal), CultureInfo.InvariantCulture);
}
