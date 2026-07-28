namespace Companion.Application.Events;

public interface IEventBus
{
    IDisposable Subscribe<T>(Action<T> handler) where T : CompanionEvent;
    void Publish<T>(T message) where T : CompanionEvent;
}
