using System.Runtime.InteropServices;
using Companion.Application.Abstractions;

namespace Companion.Infrastructure.System;

public sealed class WindowsUserActivitySource : IUserActivitySource
{
    public bool IsSessionLocked { get; private set; }

    public TimeSpan GetIdleDuration()
    {
        if (!OperatingSystem.IsWindows())
        {
            return TimeSpan.Zero;
        }

        var info = new LastInputInfo { Size = (uint)Marshal.SizeOf<LastInputInfo>() };
        if (!GetLastInputInfo(ref info))
        {
            return TimeSpan.Zero;
        }

        var elapsed = Environment.TickCount64 - info.Time;
        return TimeSpan.FromMilliseconds(Math.Max(0, elapsed));
    }

    public void SetSessionLocked(bool locked) => IsSessionLocked = locked;

    [StructLayout(LayoutKind.Sequential)]
    private struct LastInputInfo
    {
        public uint Size;
        public uint Time;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetLastInputInfo(ref LastInputInfo plii);
}
