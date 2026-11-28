using OpenCvSharp;

sealed class ScannerView : IDisposable
{
    const int Width = 640, Height = 488;
    static readonly Rect Viewport = new(16, 58, 608, 370);
    static readonly Scalar Background = Scalar.All(14);
    static readonly Scalar Border = Scalar.All(83);
    static readonly Scalar Ink = Scalar.All(236);
    static readonly Scalar Muted = Scalar.All(161);
    readonly Mat screen = new(Height, Width, MatType.CV_8UC3);
    readonly Mat gray = new();
    readonly Mat resized = new();

    public Mat Render(Mat? frame, long elapsed, string status)
    {
        screen.SetTo(Background);
        Cv2.Rectangle(screen, new Rect(1, 1, Width - 2, Height - 2), Border);
        Write(">_  TAPSEND / PHONE QR", 16, 32, Ink);
        Write("[ESC] CLOSE", 493, 32, Muted);

        if (frame != null)
        {
            Cv2.CvtColor(frame, gray, ColorConversionCodes.BGR2GRAY);
            double scale = Math.Min((double)Viewport.Width / gray.Width, (double)Viewport.Height / gray.Height);
            int width = (int)(gray.Width * scale), height = (int)(gray.Height * scale);
            Cv2.Resize(gray, resized, new Size(width, height));
            var region = new Rect((Width - width) / 2, Viewport.Y + (Viewport.Height - height) / 2, width, height);
            using var preview = new Mat(screen, region);
            Cv2.CvtColor(resized, preview, ColorConversionCodes.GRAY2BGR);
        }
        Cv2.Rectangle(screen, Viewport, Border);

        const int left = 196, right = 444, top = 119, bottom = 367, arm = 26;
        foreach (int x in new[] { left, right })
        foreach (int y in new[] { top, bottom })
        {
            var corner = new Point(x, y);
            AimLine(corner, new Point(x + (x == left ? arm : -arm), y), 2);
            AimLine(corner, new Point(x, y + (y == top ? arm : -arm)), 2);
        }
        if (frame != null)
        {
            int sweep = (int)(elapsed / 10 % ((bottom - top - 12) * 2));
            int line = top + 6 + Math.Min(sweep, (bottom - top - 12) * 2 - sweep);
            AimLine(new Point(left + 5, line), new Point(right - 5, line), 1);
        }
        Write(status, 16, 463, Ink);
        return screen;
    }

    void AimLine(Point first, Point last, int thickness)
    {
        Cv2.Line(screen, first, last, Background, thickness + 2);
        Cv2.Line(screen, first, last, Ink, thickness);
    }

    void Write(string value, int x, int y, Scalar color) =>
        Cv2.PutText(screen, value, new Point(x, y), HersheyFonts.HersheyPlain, 1, color, 1, LineTypes.AntiAlias);

    public void Dispose()
    {
        resized.Dispose();
        gray.Dispose();
        screen.Dispose();
    }
}
