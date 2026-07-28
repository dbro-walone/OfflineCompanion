using System.Globalization;
using System.Windows;
using Companion.Application.Services;

namespace Companion.App;

public partial class ReminderWindow
{
    private readonly ReminderService _service;

    public ReminderWindow(ReminderService service)
    {
        InitializeComponent();
        _service = service;
        DatePicker.SelectedDate = DateTime.Today;
    }

    private async void Save(object sender, RoutedEventArgs e)
    {
        ErrorText.Text = string.Empty;
        if (DatePicker.SelectedDate is not DateTime date ||
            !TimeOnly.TryParse(TimeBox.Text, CultureInfo.CurrentCulture, out var time))
        {
            ErrorText.Text = "请输入有效的日期和时间，例如 09:30。";
            return;
        }

        var local = date.Date + time.ToTimeSpan();
        var trigger = new DateTimeOffset(local);
        try
        {
            await _service.CreateOneTimeAsync(TitleBox.Text, trigger);
            Close();
        }
        catch (ArgumentException ex)
        {
            ErrorText.Text = ex.Message;
        }
    }

    private void Cancel(object sender, RoutedEventArgs e) => Close();
}
