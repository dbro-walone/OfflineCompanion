using System.Windows;
using System.Windows.Input;
using Companion.Domain.Entities;
using Companion.Presentation.ViewModels;
using Microsoft.Extensions.DependencyInjection;

namespace Companion.App;

public partial class TodoWindow
{
    private readonly TodoListViewModel _viewModel;
    private readonly IServiceProvider _services;
    private bool _refreshing;

    public TodoWindow(TodoListViewModel viewModel, IServiceProvider services)
    {
        InitializeComponent();
        _viewModel = viewModel;
        _services = services;
        DataContext = viewModel;
        Loaded += async (_, _) => await viewModel.RefreshAsync();
    }

    private async void OnClearCompleted(object sender, RoutedEventArgs e) =>
        await _viewModel.ClearCompletedAsync();

    private void OnPreviousPage(object sender, RoutedEventArgs e) => _viewModel.PreviousPage();

    private void OnNextPage(object sender, RoutedEventArgs e) => _viewModel.NextPage();

    private void OnStartFocus(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: TodoItem item })
        {
            return;
        }

        var timer = _services.GetRequiredService<TimerWindow>();
        timer.Owner = this;
        timer.FocusOn(item);
        timer.Closed += async (_, _) => await _viewModel.RefreshAsync();
        timer.Show();
    }

    private void OnTitleKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter && _viewModel.AddCommand.CanExecute(null))
        {
            _viewModel.AddCommand.Execute(null);
            e.Handled = true;
        }
        else if (e.Key == Key.Escape)
        {
            Close();
        }
    }

    private async void OnTodoToggled(object sender, RoutedEventArgs e)
    {
        if (_refreshing || sender is not FrameworkElement { Tag: TodoItem item })
        {
            return;
        }

        _refreshing = true;
        try
        {
            await _viewModel.ToggleAsync(item);
        }
        finally
        {
            _refreshing = false;
        }
    }
}
