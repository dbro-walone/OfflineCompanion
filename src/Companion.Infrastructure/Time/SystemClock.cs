using Companion.Application.Abstractions;

namespace Companion.Infrastructure.Time;

public sealed class SystemClock : IClock
{
    public DateTimeOffset Now => DateTimeOffset.Now;
}
