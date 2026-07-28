using System.Windows;
using System.Windows.Threading;
using Companion.Application.Abstractions;
using Companion.Application.Events;
using Companion.Application.Services;
using Companion.Domain.Scheduling;
using Companion.Infrastructure.Config;
using Companion.Infrastructure.Logging;
using Companion.Infrastructure.Paths;
using Companion.Infrastructure.Storage;
using Companion.Infrastructure.System;
using Companion.Infrastructure.Time;
using Companion.Packages.Installation;
using Companion.Packages.Validation;
using Companion.Presentation.ViewModels;
using Microsoft.Data.Sqlite;
using Microsoft.Extensions.DependencyInjection;

namespace Companion.App;

public partial class App
{
    private ServiceProvider? _services;
    private DispatcherTimer? _scheduler;
    private IDisposable? _reminderSubscription;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        try
        {
            var services = new ServiceCollection();
            var paths = new AppDataPaths();
            paths.EnsureCreated();
            var seeder = new BundledPackageSeeder(paths);
            await seeder.SeedAsync();

            var configStore = new JsonConfigStore(paths.Settings);
            var settings = await configStore.LoadAsync();
            ThemeManager.Apply(settings.Theme);
            var connectionString = new SqliteConnectionStringBuilder
            {
                DataSource = paths.Database,
                Mode = SqliteOpenMode.ReadWriteCreate,
                Cache = SqliteCacheMode.Shared
            }.ToString();

            services.AddSingleton(paths);
            services.AddSingleton(configStore);
            services.AddSingleton(settings);
            services.AddSingleton<IClock, SystemClock>();
            services.AddSingleton<IUserActivitySource, WindowsUserActivitySource>();
            services.AddSingleton<IEventBus, InProcessEventBus>();
            services.AddSingleton<ICompanionStore>(_ => new SqliteCompanionStore(connectionString));
            services.AddSingleton<ReminderCalculator>();
            services.AddSingleton<ManifestValidator>();
            services.AddSingleton<SafePackageInstaller>();
            services.AddSingleton<LocalLogService>();
            services.AddSingleton<TodoService>();
            services.AddSingleton<ReminderService>();
            services.AddSingleton<PomodoroService>();
            services.AddSingleton<SedentaryService>();
            services.AddTransient<TodoListViewModel>();
            services.AddTransient<TodoWindow>();
            services.AddTransient<ReminderWindow>();
            services.AddTransient<TimerWindow>();
            services.AddTransient<SettingsWindow>(provider => new SettingsWindow(
                provider.GetRequiredService<AppSettings>(),
                provider.GetRequiredService<JsonConfigStore>(),
                provider.GetRequiredService<AppDataPaths>(),
                provider.GetRequiredService<IEventBus>()));
            services.AddTransient<PackageManagerWindow>();
            services.AddSingleton<PetWindow>();

            _services = services.BuildServiceProvider();
            await _services.GetRequiredService<ICompanionStore>().InitializeAsync();

            var petWindow = _services.GetRequiredService<PetWindow>();
            MainWindow = petWindow;
            petWindow.Show();
            ConfigureScheduler();
            ConfigureNotifications(petWindow);
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                $"鸦影启动失败：{ex.Message}",
                "离线桌面陪伴助手",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown(1);
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _scheduler?.Stop();
        _reminderSubscription?.Dispose();
        _services?.Dispose();
        base.OnExit(e);
    }

    private void ConfigureScheduler()
    {
        if (_services is null)
        {
            return;
        }

        var reminderService = _services.GetRequiredService<ReminderService>();
        _scheduler = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _scheduler.Tick += async (_, _) =>
        {
            _scheduler.Stop();
            try
            {
                await reminderService.CheckDueAsync();
            }
            finally
            {
                _scheduler.Start();
            }
        };
        _scheduler.Start();
    }

    private void ConfigureNotifications(PetWindow petWindow)
    {
        if (_services is null)
        {
            return;
        }

        var eventBus = _services.GetRequiredService<IEventBus>();
        _reminderSubscription = eventBus.Subscribe<ReminderDue>(message =>
        {
            Dispatcher.Invoke(() =>
            {
                var notification = new NotificationWindow(message.Title)
                {
                    Owner = petWindow
                };
                notification.PositionNear(petWindow);
                notification.Show();
            });
        });
    }
}
