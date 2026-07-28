using System.Windows;
using System.Windows.Input;
using Companion.Domain.Entities;
using Companion.Presentation.ViewModels;

namespace Companion.App;

public partial class TodoWindow
{
    private readonly TodoListViewModel _viewModel;
    private bool _refreshing;

    public TodoWindow(TodoListViewModel viewModel)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = viewModel;
        Loaded += async (_, _) => await viewModel.RefreshAsync();
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
