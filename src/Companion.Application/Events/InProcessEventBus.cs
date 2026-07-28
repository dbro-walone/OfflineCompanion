using System.Collections.Concurrent;

namespace Companion.Application.Events;

public sealed class InProcessEventBus : IEventBus
{
    private readonly ConcurrentDictionary<Type, List<Delegate>> _handlers = new();

    public IDisposable Subscribe<T>(Action<T> handler) where T : CompanionEvent
    {
        var handlers = _handlers.GetOrAdd(typeof(T), _ => []);
        lock (handlers)
        {
            handlers.Add(handler);
        }

        return new Subscription(() =>
        {
            lock (handlers)
            {
                handlers.Remove(handler);
            }
        });
    }

    public void Publish<T>(T message) where T : CompanionEvent
    {
        if (!_handlers.TryGetValue(typeof(T), out var handlers))
        {
            return;
        }

        Delegate[] snapshot;
        lock (handlers)
        {
            snapshot = handlers.ToArray();
        }

        foreach (var handler in snapshot.Cast<Action<T>>())
        {
            handler(message);
        }
    }

    private sealed class Subscription(Action dispose) : IDisposable
    {
        private Action? _dispose = dispose;
        public void Dispose() => Interlocked.Exchange(ref _dispose, null)?.Invoke();
    }
}
