using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace Companion.App;

public static class MonitorPlacement
{
    private const uint MonitorDefaultToNearest = 2;

    public static Rect GetWorkArea(Window window)
    {
        if (!OperatingSystem.IsWindows())
        {
            return SystemParameters.WorkArea;
        }

        var handle = new WindowInteropHelper(window).Handle;
        var monitor = MonitorFromWindow(handle, MonitorDefaultToNearest);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        if (monitor == IntPtr.Zero || !GetMonitorInfo(monitor, ref info))
        {
            return SystemParameters.WorkArea;
        }

        return new Rect(
            info.Work.Left,
            info.Work.Top,
            info.Work.Right - info.Work.Left,
            info.Work.Bottom - info.Work.Top);
    }

    public static string? ClampAndDetectEdge(Window window, double threshold = 12)
    {
        var work = GetWorkArea(window);
        window.Left = Math.Clamp(window.Left, work.Left, Math.Max(work.Left, work.Right - window.Width));
        window.Top = Math.Clamp(window.Top, work.Top, Math.Max(work.Top, work.Bottom - window.Height));

        if (Math.Abs(window.Left - work.Left) <= threshold)
        {
            return "edge.left";
        }

        if (Math.Abs(window.Left + window.Width - work.Right) <= threshold)
        {
            return "edge.right";
        }

        if (Math.Abs(window.Top - work.Top) <= threshold)
        {
            return "edge.top";
        }

        if (Math.Abs(window.Top + window.Height - work.Bottom) <= threshold)
        {
            return "edge.bottom";
        }

        return null;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MonitorInfo
    {
        public int Size;
        public NativeRect Monitor;
        public NativeRect Work;
        public uint Flags;
    }

    [DllImport("user32.dll")]
    private static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint flags);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
}
