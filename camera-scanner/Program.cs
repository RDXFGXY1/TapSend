using OpenCvSharp;
using System.Net;
using System.Text;
using System.Text.Json;

// Exit codes: 0 scanned, 1 cancelled, 2 camera unavailable.
try
{
    if (args.Length == 4 && args[0] == "--screen")
        return ScreenViewer.Run(args[1], args[2], args[3]);

    if (args.Length >= 2 && args[0] == "--list")
    {
        if (!IsPhoneShare(args[1])) return 2;
        using var client = PhoneClient();
        using var response = client.GetAsync(args[1].TrimEnd('/') + "/manifest").GetAwaiter().GetResult();
        if (response.StatusCode == HttpStatusCode.Accepted) return 3;
        response.EnsureSuccessStatusCode();
        byte[] body = response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
        if (body.Length > 1024 * 1024) throw new IOException("Phone file list is too large.");
        using var json = JsonDocument.Parse(body);
        if (json.RootElement.ValueKind != JsonValueKind.Array || json.RootElement.GetArrayLength() > 1000)
            throw new IOException("Invalid phone file list.");
        foreach (var file in json.RootElement.EnumerateArray())
        {
            int id = file.GetProperty("id").GetInt32();
            string name = file.GetProperty("name").GetString() ?? "file";
            long size = file.GetProperty("size").GetInt64();
            if (id is < 0 or > 99999 || name.Length > 1024) throw new IOException("Invalid phone file entry.");
            Console.WriteLine($"{id}\t{Convert.ToHexString(Encoding.UTF8.GetBytes(name))}\t{size}");
        }
        return 0;
    }
    if (args.Length == 6 && args[0] == "--download")
    {
        if (!IsPhoneShare(args[1]) || !int.TryParse(args[2], out int id) || id is < 0 or > 99999)
            return 2;
        string name = Encoding.UTF8.GetString(Convert.FromHexString(args[3]));
        if (!long.TryParse(args[4], out long expectedSize)) return 2;
        string folder = args[5];
        using var client = PhoneClient();
        using var response = client.GetAsync(args[1].TrimEnd('/') + "/file/" + id,
            HttpCompletionOption.ResponseHeadersRead).GetAwaiter().GetResult();
        if (response.StatusCode == HttpStatusCode.Accepted) return 3;
        response.EnsureSuccessStatusCode();
        Directory.CreateDirectory(folder);
        string safeName = SafeName(name);
        bool existing = Path.Exists(Path.Combine(folder, safeName));
        if (existing) Console.Error.WriteLine("EXISTS\t" + safeName);
        string temporary = Path.Combine(folder, "incoming-" + Guid.NewGuid().ToString("N") + ".part");
        try
        {
            using (var destination = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            using (var source = response.Content.ReadAsStreamAsync().GetAwaiter().GetResult())
            {
                byte[] buffer = new byte[65536];
                long total = 0;
                long lastReport = 0;
                var clock = System.Diagnostics.Stopwatch.StartNew();
                int count;
                while ((count = source.Read(buffer)) != 0)
                {
                    total += count;
                    if (total > 8L * 1024 * 1024 * 1024) throw new IOException("File exceeds the 8 GiB download limit.");
                    destination.Write(buffer, 0, count);
                    if (clock.ElapsedMilliseconds - lastReport >= 150)
                    {
                        Console.Error.WriteLine($"PROGRESS\t{total}\t{expectedSize}");
                        lastReport = clock.ElapsedMilliseconds;
                    }
                }
                if (response.Content.Headers.ContentLength is long length && length != total)
                    throw new IOException("Phone transfer ended before the full file arrived.");
                if (expectedSize >= 0 && expectedSize != total)
                    throw new IOException("Phone file size changed or transfer ended early.");
            }
            Console.Error.WriteLine($"PROGRESS\t{(expectedSize < 0 ? new FileInfo(temporary).Length : expectedSize)}\t{expectedSize}");
            string saved = SaveUnique(temporary, folder, safeName);
            if (!string.Equals(saved, Path.Combine(folder, safeName), StringComparison.Ordinal))
                Console.Error.WriteLine("RENAMED\t" + safeName);
            if (response.Headers.TryGetValues("X-TapSend-Transfer", out var values)
                && long.TryParse(values.FirstOrDefault(), out long transfer) && transfer > 0)
            {
                try
                {
                    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(3));
                    using var receipt = client.PostAsync(args[1].TrimEnd('/') + $"/receipt/{id}/{transfer}",
                        null, timeout.Token).GetAwaiter().GetResult();
                }
                catch (HttpRequestException) { }
                catch (OperationCanceledException) { }
            }
            Console.WriteLine(saved);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
        return 0;
    }
    if (args.Length == 2 && args[0] == "--scan-file")
    {
        using var image = Cv2.ImRead(args[1]);
        using var reader = new QRCodeDetector();
        string decoded = reader.DetectAndDecode(image, out _);
        if (!IsPhoneShare(decoded)) return 2;
        Console.WriteLine(decoded.Trim());
        return 0;
    }
    using var camera = new VideoCapture(0, VideoCaptureAPIs.DSHOW);
    if (!camera.IsOpened())
    {
        Console.Error.WriteLine("No laptop camera found. Check Windows camera privacy settings.");
        return 2;
    }
    camera.Set(VideoCaptureProperties.FrameWidth, 640);
    camera.Set(VideoCaptureProperties.FrameHeight, 480);
    using var frame = new Mat();
    using var detector = new QRCodeDetector();
    using var scannerView = new ScannerView();
    const string title = "TapSend / QR scanner";
    Cv2.NamedWindow(title, WindowFlags.AutoSize);
    var scanClock = System.Diagnostics.Stopwatch.StartNew();
    bool wrongCode = false;
    while (true)
    {
        bool hasFrame = camera.Read(frame) && !frame.Empty();
        string value = hasFrame ? detector.DetectAndDecode(frame, out _) : "";
        if (IsPhoneShare(value))
        {
            Cv2.ImShow(title, scannerView.Render(frame, scanClock.ElapsedMilliseconds, "[+] TAPSEND QR FOUND"));
            Cv2.WaitKey(180);
            Console.WriteLine(value.Trim());
            return 0;
        }
        if (!string.IsNullOrEmpty(value)) wrongCode = true;
        string status = !hasFrame ? "[ ] WAITING FOR CAMERA"
            : wrongCode ? "[!] OPEN SHARE ON THE PHONE AND SCAN ITS QR"
            : "[~] ALIGN THE PHONE QR IN THE FRAME";
        Cv2.ImShow(title, scannerView.Render(hasFrame ? frame : null, scanClock.ElapsedMilliseconds, status));
        if (Cv2.WaitKey(30) == 27 || Cv2.GetWindowProperty(title, WindowPropertyFlags.Visible) < 1) return 1;
    }
}
catch (Exception error)
{
    Console.Error.WriteLine((args.FirstOrDefault() == "--download" ? "Could not save file: " : "Camera scanner unavailable: ") + error.Message);
    return 2;
}
finally
{
    Cv2.DestroyAllWindows();
}

static bool IsPhoneShare(string? value)
{
    if (!Uri.TryCreate(value?.Trim(), UriKind.Absolute, out var uri) || uri.Scheme != "http" ||
        uri.UserInfo.Length != 0 || uri.Query.Length != 0 || uri.Fragment.Length != 0 ||
        uri.Port < 1 || uri.Port > 65535 ||
        !System.Text.RegularExpressions.Regex.IsMatch(uri.AbsolutePath, @"^/s/[0-9a-f]{48}/?$"))
        return false;
    return System.Net.IPAddress.TryParse(uri.Host, out var ip) &&
           ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork &&
           ip.GetAddressBytes() is var octets &&
           (octets[0] == 10 || octets[0] == 172 && octets[1] is >= 16 and <= 31 ||
            octets[0] == 192 && octets[1] == 168);
}

static HttpClient PhoneClient()
{
    var client = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false });
    client.Timeout = TimeSpan.FromMinutes(5);
    return client;
}

static string SafeName(string name)
{
    string result = new string(name.Select(c => Path.GetInvalidFileNameChars().Contains(c) || char.IsControl(c) ? '_' : c).ToArray()).Trim(' ', '.');
    return string.IsNullOrWhiteSpace(result) ? "phone-file" : result[..Math.Min(result.Length, 160)];
}

static string SaveUnique(string temporary, string folder, string name)
{
    string stem = Path.GetFileNameWithoutExtension(name), ext = Path.GetExtension(name);
    for (int index = 1; index <= 9999; index++)
    {
        string path = Path.Combine(folder, index == 1 ? name : $"{stem} ({index}){ext}");
        if (Path.Exists(path)) continue;
        try
        {
            File.Move(temporary, path);
            return path;
        }
        catch (IOException) when (Path.Exists(path)) { }
    }
    throw new IOException("Too many files with the same name in the inbox.");
}
