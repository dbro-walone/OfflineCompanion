using System.Windows;

namespace Companion.App;

public partial class NotificationWindow
{
    public NotificationWindow(string message)
    {
        InitializeComponent();
        MessageText.Text = message;
        Deactivated += (_, _) =>
        {
            if (!IsKeyboardFocusWithin)
            {
                Topmost = true;
            }
        };
    }

    public void PositionNear(Window owner)
    {
        Left = Math.Max(SystemParameters.WorkArea.Left, owner.Left - Width - 12);
        Top = Math.Min(
            SystemParameters.WorkArea.Bottom - Height,
            Math.Max(SystemParameters.WorkArea.Top, owner.Top + owner.Height - Height));
    }

    private void Snooze(object sender, RoutedEventArgs e) => Close();
    private void Dismiss(object sender, RoutedEventArgs e) => Close();
}
