using OpenCvSharp;
using System.Net;
using System.Net.Http.Json;
using System.Security.Cryptography;
using System.Text;

static class ScreenViewer
{
    const string Title = "TapSend / Live screen";
    const int Width = 900, Height = 650;
    static readonly Scalar Ink = Scalar.All(237);
    static readonly Scalar Panel = Scalar.All(16);
    static readonly Scalar Border = Scalar.All(84);
    static readonly Rect Viewport = new(16, 58, 868, 526);

    sealed record View(string Status, byte[]? Image, bool Closed = false);

    public static int Run(string portValue, string key, string pinText)
    {
        if (!int.TryParse(portValue, out int port) || port is < 1 or > 65535 ||
            key.Length != 64 || pinText.Length != 64) return 2;
        byte[] pin = Convert.FromHexString(pinText);
        if (Convert.FromHexString(key).Length != 32) return 2;
        using var handler = new HttpClientHandler { UseProxy = false, AllowAutoRedirect = false };
        handler.ServerCertificateCustomValidationCallback = (_, cert, _, _) =>
            cert != null && CryptographicOperations.FixedTimeEquals(SHA256.HashData(cert.RawData), pin);
        using var client = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(2) };
        client.DefaultRequestHeaders.Add("X-TapSend-View", key);
        using var stop = new CancellationTokenSource();
        View current = new("WAITING FOR PHONE", null);
        View? painted = null;
        Task worker = Poll(client, port, view => Volatile.Write(ref current, view), stop.Token);
        using var canvas = new Mat(Height, Width, MatType.CV_8UC3);
        using var scaled = new Mat();
        Cv2.NamedWindow(Title, WindowFlags.AutoSize);
        View? frozen = null;
        Rect imageBounds = default, selection = default;
        Point? anchor = null;
        Point? arrowStart = null, arrowTip = null;
        Task<string>? grab = null;
        string message = "";
        bool redraw = false;
        MouseCallback mouse = (action, x, y, _, _) =>
        {
            if (imageBounds.Width == 0) return;
            var point = new Point(Math.Clamp(x, imageBounds.Left, imageBounds.Right - 1),
                Math.Clamp(y, imageBounds.Top, imageBounds.Bottom - 1));
            if (frozen == null)
            {
                if (painted?.Image == null) return;
                if (action == MouseEventTypes.RButtonDown && imageBounds.Contains(x, y))
                    arrowStart = arrowTip = point;
                if (arrowStart is Point origin && action is MouseEventTypes.MouseMove or MouseEventTypes.RButtonUp)
                {
                    arrowTip = point;
                    if (action == MouseEventTypes.RButtonUp)
                    {
                        if (grab == null && (Math.Abs(origin.X - point.X) + Math.Abs(origin.Y - point.Y)) >= 5)
                            grab = SendArrow(client, port, imageBounds, origin, point, stop.Token);
                        arrowStart = arrowTip = null;
                    }
                    redraw = true;
                }
                return;
            }
            if (action == MouseEventTypes.LButtonDown && imageBounds.Contains(x, y))
            {
                anchor = point;
                selection = default;
                redraw = true;
            }
            if (anchor is Point start && action is MouseEventTypes.MouseMove or MouseEventTypes.LButtonUp)
            {
                selection = new Rect(Math.Min(start.X, point.X), Math.Min(start.Y, point.Y),
                    Math.Abs(start.X - point.X) + 1, Math.Abs(start.Y - point.Y) + 1);
                if (action == MouseEventTypes.LButtonUp) anchor = null;
                redraw = true;
            }
        };
        Cv2.SetMouseCallback(Title, mouse);
        try
        {
            while (true)
            {
                View live = Volatile.Read(ref current);
                if (live.Closed) return 0;
                if (grab is { IsCompleted: true })
                {
                    message = grab.IsCompletedSuccessfully ? grab.Result : "COULD NOT ADD TO PASTE STACK";
                    grab = null;
                    redraw = true;
                }
                View frame = frozen ?? live;
                if (redraw || !ReferenceEquals(frame, painted))
                {
                    imageBounds = Render(canvas, scaled, frame, frozen != null, message);
                    if (frozen != null && selection.Width > 1 && selection.Height > 1)
                        Cv2.Rectangle(canvas, selection, Ink, 2);
                    if (frame.Image != null && arrowStart is Point origin && arrowTip is Point tip)
                        Cv2.ArrowedLine(canvas, origin, tip, new Scalar(48, 36, 255), 2, LineTypes.AntiAlias);
                    Cv2.ImShow(Title, canvas);
                    painted = frame;
                    redraw = false;
                }
                int keyPress = Cv2.WaitKey(8);
                if (keyPress == 27 || Cv2.GetWindowProperty(Title, WindowPropertyFlags.Visible) < 1) return 0;
                if (keyPress == ' ' && (frozen != null || painted?.Image != null))
                {
                    frozen = frozen == null ? painted : null;
                    selection = default;
                    anchor = null;
                    arrowStart = arrowTip = null;
                    message = "";
                    redraw = true;
                }
                if (grab == null && frozen?.Image is byte[] jpeg && keyPress is 'g' or 'G' or 't' or 'T')
                {
                    bool text = keyPress is 't' or 'T';
                    byte[] crop = Crop(jpeg, imageBounds, selection);
                    if (crop.Length == 0) continue;
                    message = text ? "READING TEXT..." : "SAVING GRAB...";
                    redraw = true;
                    grab = SaveGrab(client, port, crop, text, stop.Token);
                }
            }
        }
        finally
        {
            stop.Cancel();
            worker.GetAwaiter().GetResult();
            if (grab != null)
                try { grab.GetAwaiter().GetResult(); } catch (OperationCanceledException) { }
            GC.KeepAlive(mouse);
        }
    }

    static async Task<string> SendArrow(HttpClient client, int port, Rect bounds, Point from, Point to, CancellationToken stop)
    {
        int X(Point point) => 1000 * (point.X - bounds.Left) / Math.Max(1, bounds.Width - 1);
        int Y(Point point) => 1000 * (point.Y - bounds.Top) / Math.Max(1, bounds.Height - 1);
        try
        {
            using var response = await client.PostAsJsonAsync($"https://127.0.0.1:{port}/screen/arrow",
                new[] { X(from), Y(from), X(to), Y(to) }, stop);
            return response.StatusCode switch
            {
                HttpStatusCode.NoContent => "ARROW SENT",
                HttpStatusCode.Conflict => "ON ANDROID: TOOLS > SHARE SCREEN WITH LIVE RED PEN",
                _ => "COULD NOT SEND ARROW"
            };
        }
        catch (Exception error) when (error is HttpRequestException or OperationCanceledException)
        {
            return "COULD NOT SEND ARROW";
        }
    }

    static byte[] Crop(byte[] jpeg, Rect bounds, Rect selection)
    {
        if (bounds.Width == 0 || bounds.Height == 0) return [];
        using var image = Cv2.ImDecode(jpeg, ImreadModes.Color);
        if (image.Empty()) return [];
        if (selection.Width < 2 || selection.Height < 2) selection = bounds;
        int left = (selection.Left - bounds.Left) * image.Width / bounds.Width;
        int top = (selection.Top - bounds.Top) * image.Height / bounds.Height;
        int right = Math.Min(image.Width, (int)Math.Ceiling((selection.Right - bounds.Left) * (double)image.Width / bounds.Width));
        int bottom = Math.Min(image.Height, (int)Math.Ceiling((selection.Bottom - bounds.Top) * (double)image.Height / bounds.Height));
        using var cropped = new Mat(image, new Rect(left, top, right - left, bottom - top));
        return Cv2.ImEncode(".png", cropped, out byte[] bytes) ? bytes : [];
    }

    static async Task<string> SaveGrab(HttpClient client, int port, byte[] crop, bool text, CancellationToken stop)
    {
        try
        {
            byte[] data = crop;
            if (text)
            {
                string extracted = await ScreenOcr.Read(crop, stop);
                if (string.IsNullOrWhiteSpace(extracted)) return "NO TEXT FOUND";
                data = Encoding.UTF8.GetBytes(extracted);
                if (data.Length > 16 * 1024) return "TOO MUCH TEXT - SELECT A SMALLER AREA";
            }
            using var body = new ByteArrayContent(data);
            using var response = await client.PostAsync($"https://127.0.0.1:{port}/screen/grab/{(text ? "text" : "image")}", body, stop);
            return response.StatusCode switch
            {
                HttpStatusCode.NoContent => "SAVED TO PASTE STACK / SHIFT+V ON DESKTOP",
                HttpStatusCode.Conflict => "PASTE STACK FULL / SHIFT+V ON DESKTOP",
                _ => "COULD NOT ADD TO PASTE STACK"
            };
        }
        catch (Exception error) when (error is HttpRequestException or OperationCanceledException or IOException or System.ComponentModel.Win32Exception)
        {
            return text ? "WINDOWS TEXT RECOGNITION UNAVAILABLE" : "COULD NOT ADD TO PASTE STACK";
        }
    }

    static async Task Poll(HttpClient client, int port, Action<View> publish, CancellationToken stop)
    {
        ulong sequence = 0;
        string phase = "";
        string status = "WAITING FOR PHONE";
        int missed = 0;
        while (!stop.IsCancellationRequested)
        {
            try
            {
                using var response = await client.GetAsync(
                    $"https://127.0.0.1:{port}/screen?after={sequence}&wait=1&phase={phase}", stop);
                if (response.StatusCode == HttpStatusCode.Gone)
                {
                    publish(new View("VIEWER CLOSED", null, true));
                    return;
                }
                response.EnsureSuccessStatusCode();
                missed = 0;
                phase = response.Headers.TryGetValues("X-TapSend-Screen", out var values)
                    ? values.FirstOrDefault() switch { "sharing" => "sharing", "stopped" => "stopped", _ => "waiting" }
                    : "waiting";
                string next = phase switch
                {
                    "sharing" => "LIVE / ANDROID",
                    "stopped" => "PHONE STOPPED SHARING",
                    _ => "WAITING FOR PHONE"
                };
                if (response.StatusCode == HttpStatusCode.OK &&
                    response.Headers.TryGetValues("X-TapSend-Frame", out var frame) &&
                    ulong.TryParse(frame.FirstOrDefault(), out ulong received))
                {
                    byte[] jpeg = await response.Content.ReadAsByteArrayAsync(stop);
                    if (jpeg.Length is > 0 and <= 512 * 1024)
                    {
                        sequence = received;
                        publish(new View(next, jpeg));
                    }
                }
                else if (next != status) publish(new View(next, null));
                status = next;
                if (response.Headers.Contains("X-TapSend-Screen-Wait")) continue;
            }
            catch (Exception error) when (error is HttpRequestException or OperationCanceledException)
            {
                if (stop.IsCancellationRequested) return;
                status = "CONNECTION LOST";
                publish(new View(status, null, ++missed >= 3));
                if (missed >= 3) return;
            }
            try { await Task.Delay(110, stop); }
            catch (OperationCanceledException) { return; }
        }
    }

    static Rect Render(Mat canvas, Mat scaled, View frame, bool frozen, string message)
    {
        canvas.SetTo(Panel);
        Cv2.Rectangle(canvas, new Rect(1, 1, Width - 2, Height - 2), Border);
        Cv2.Rectangle(canvas, Viewport, Border);
        Write(canvas, ">_  TAPSEND / SCREEN SHARE", 17, 33);
        Write(canvas, "[ESC] CLOSE", 765, 33);
        Write(canvas, message.Length > 0 ? message : frozen ? "FROZEN / DRAG TO SELECT" : frame.Status, 17, 612);
        Write(canvas, frozen ? "[SPACE] LIVE   [G] SAVE IMAGE   [T] READ TEXT" : "[SPACE] FREEZE & GRAB   [RIGHT-DRAG] RED PEN", 17, 636);
        if (frame.Image == null)
        {
            string hint = frame.Status == "WAITING FOR PHONE"
                ? "ON THE PHONE: TOOLS > SHARE SCREEN TO PC"
                : "START A NEW SHARE FROM TAPSend ON THE PHONE";
            Write(canvas, hint, 215, Height / 2);
            return default;
        }
        using var image = Cv2.ImDecode(frame.Image, ImreadModes.Color);
        if (image.Empty()) return default;
        double scale = Math.Min((double)(Viewport.Width - 4) / image.Width,
            (double)(Viewport.Height - 4) / image.Height);
        int w = Math.Max(1, (int)(image.Width * scale)), h = Math.Max(1, (int)(image.Height * scale));
        Cv2.Resize(image, scaled, new Size(w, h), interpolation: InterpolationFlags.Linear);
        var bounds = new Rect(Viewport.X + (Viewport.Width - w) / 2, Viewport.Y + (Viewport.Height - h) / 2, w, h);
        using var area = new Mat(canvas, bounds);
        scaled.CopyTo(area);
        return bounds;
    }

    static void Write(Mat canvas, string text, int x, int y) =>
        Cv2.PutText(canvas, text, new Point(x, y), HersheyFonts.HersheyPlain, 1.2, Ink, 1, LineTypes.AntiAlias);
}
