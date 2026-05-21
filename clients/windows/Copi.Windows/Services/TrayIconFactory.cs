using Copi.Windows.Models;
using Drawing = System.Drawing;
using Drawing2D = System.Drawing.Drawing2D;
using RuntimeInterop = System.Runtime.InteropServices;

namespace Copi.Windows.Services;

public static class TrayIconFactory
{
    public static Drawing.Icon Create(ProcessStatus status)
    {
        using var bitmap = new Drawing.Bitmap(32, 32);
        using var graphics = Drawing.Graphics.FromImage(bitmap);
        graphics.SmoothingMode = Drawing2D.SmoothingMode.AntiAlias;
        graphics.Clear(Drawing.Color.Transparent);

        using var brush = new Drawing.SolidBrush(ColorFor(status));
        using var outline = new Drawing.Pen(Drawing.Color.FromArgb(90, 0, 0, 0), 1.4f);
        var cloud = new Drawing2D.GraphicsPath();
        cloud.AddEllipse(5, 14, 10, 10);
        cloud.AddEllipse(10, 8, 13, 14);
        cloud.AddEllipse(19, 12, 9, 10);
        cloud.AddRectangle(new Drawing.RectangleF(8, 17, 18, 8));
        graphics.FillPath(brush, cloud);
        graphics.DrawPath(outline, cloud);

        if (status == ProcessStatus.Failed)
        {
            using var pen = new Drawing.Pen(Drawing.Color.White, 2.5f)
            {
                StartCap = Drawing2D.LineCap.Round,
                EndCap = Drawing2D.LineCap.Round,
            };
            graphics.DrawLine(pen, 12, 13, 21, 22);
            graphics.DrawLine(pen, 21, 13, 12, 22);
        }

        var handle = bitmap.GetHicon();
        try
        {
            return (Drawing.Icon)Drawing.Icon.FromHandle(handle).Clone();
        }
        finally
        {
            _ = DestroyIcon(handle);
        }
    }

    private static Drawing.Color ColorFor(ProcessStatus status)
    {
        return status switch
        {
            ProcessStatus.Running => Drawing.Color.FromArgb(74, 188, 133),
            ProcessStatus.Starting or ProcessStatus.Stopping => Drawing.Color.FromArgb(238, 169, 67),
            ProcessStatus.Failed => Drawing.Color.FromArgb(220, 74, 74),
            _ => Drawing.Color.FromArgb(112, 118, 126),
        };
    }

    [RuntimeInterop.DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr hIcon);
}
