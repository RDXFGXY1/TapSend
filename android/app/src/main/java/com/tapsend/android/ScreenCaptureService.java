package com.tapsend.android;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.Image;
import android.media.ImageReader;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import android.os.Looper;
import android.os.SystemClock;
import android.view.WindowManager;
import android.widget.Toast;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;

public final class ScreenCaptureService extends Service {
    private static final String CHANNEL = "tapsend-screen";
    private static final String STOP = "com.tapsend.android.STOP_SCREEN_SHARE";
    static final String CONSENT = "screen-consent";
    static final String RED_PEN = "screen-red-pen";
    private static final byte[] HEARTBEAT = new byte[0];
    private static final int MAX_SIDE = 960;
    // Allow for display scan timing jitter around a 30 FPS capture cadence.
    private static final long MIN_FRAME_INTERVAL_MS = 30;
    private static volatile boolean active;

    private final Handler main = new Handler(Looper.getMainLooper());
    private final ExecutorService sender = Executors.newSingleThreadExecutor();
    private final ArrayBlockingQueue<byte[]> latest = new ArrayBlockingQueue<>(1);
    private final String captureId = UUID.randomUUID().toString().replace("-", "");
    private volatile TapSendClient client;
    private volatile boolean running;
    private MediaProjection projection;
    private MediaProjection.Callback callback;
    private VirtualDisplay display;
    private volatile ImageReader reader;
    private HandlerThread imageThread;
    private Handler images;
    // Used exclusively on the image thread.
    private Bitmap packed, cropped;
    private ByteBuffer padded;
    private final ByteArrayOutputStream encoded = new ByteArrayOutputStream();
    private long lastFrame;
    private ScreenArrow arrow;

    static boolean isActive() { return active; }

    @Override public IBinder onBind(Intent intent) { return null; }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        if (running) return START_NOT_STICKY;
        Intent consent = intent == null ? null : intent.getParcelableExtra(CONSENT, Intent.class);
        if (consent == null) {
            stopSelf();
            return START_NOT_STICKY;
        }
        try {
            startForeground(71, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
        } catch (RuntimeException error) {
            fail("Android could not start the screen-sharing service.");
            return START_NOT_STICKY;
        }
        active = running = true;
        boolean redPen = intent.getBooleanExtra(RED_PEN, false) && android.provider.Settings.canDrawOverlays(this);
        sender.execute(() -> send(consent, redPen));
        return START_NOT_STICKY;
    }

    private Notification notification() {
        NotificationManager notifications = getSystemService(NotificationManager.class);
        notifications.createNotificationChannel(new NotificationChannel(CHANNEL, "Screen sharing", NotificationManager.IMPORTANCE_LOW));
        PendingIntent stop = PendingIntent.getService(this, 0, new Intent(this, ScreenCaptureService.class).setAction(STOP),
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class),
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_menu_view).setContentTitle("TapSend screen sharing")
                .setContentText("Sharing with your paired computer").setContentIntent(open).setOngoing(true)
                .addAction(new Notification.Action.Builder(null, "Stop sharing", stop).build()).build();
    }

    private void send(Intent consent, boolean redPen) {
        TapSendClient phone = new TapSendClient(new IdentityStore(this));
        client = phone;
        try {
            phone.loadAndAuthenticate();
            if (!phone.hasIdentity()) throw new IllegalStateException("Pair a computer first.");
            if (!running) return;
            phone.startScreen(captureId, redPen);
            main.post(() -> startCapture(consent));
            while (running) {
                byte[] frame = latest.poll(redPen ? 200 : 1000, TimeUnit.MILLISECONDS);
                if (!running) break;
                String mark = phone.sendScreenFrame(captureId, frame == null ? HEARTBEAT : frame);
                if (redPen && mark != null) main.post(() -> showArrow(mark));
            }
        } catch (Exception error) {
            if (running) {
                String detail = error.getMessage();
                main.post(() -> { if (running) fail(detail == null ? "Screen sharing disconnected." : detail.trim()); });
            }
        } finally {
            if (phone.hasIdentity()) phone.stopScreen(captureId);
            client = null;
        }
    }

    private void showArrow(String mark) {
        if (!running) return;
        if (arrow == null) arrow = new ScreenArrow(this);
        arrow.show(mark);
    }

    private void startCapture(Intent consent) {
        if (!running) return;
        try {
            projection = getSystemService(MediaProjectionManager.class).getMediaProjection(Activity.RESULT_OK, consent);
            if (projection == null) throw new IllegalStateException("Screen sharing permission ended.");
            imageThread = new HandlerThread("tapsend-screen-frames");
            imageThread.start();
            images = new Handler(imageThread.getLooper());
            callback = new MediaProjection.Callback() {
                @Override public void onStop() { stopSelf(); }
                @Override public void onCapturedContentResize(int width, int height) {
                    if (running && display != null) resizeCapture(width, height);
                }
            };
            projection.registerCallback(callback, main);
            Rect bounds = getSystemService(WindowManager.class).getMaximumWindowMetrics().getBounds();
            resizeCapture(bounds.width(), bounds.height());
            Toast.makeText(this, "Screen sharing started", Toast.LENGTH_SHORT).show();
        } catch (RuntimeException error) {
            fail("Could not capture the screen. Try starting a new share.");
        }
    }

    private void resizeCapture(int width, int height) {
        if (width <= 0 || height <= 0) return;
        double scale = Math.min(1.0, (double)MAX_SIDE / Math.max(width, height));
        int w = Math.max(1, (int)Math.round(width * scale)), h = Math.max(1, (int)Math.round(height * scale));
        ImageReader old = reader;
        if (old != null && old.getWidth() == w && old.getHeight() == h) return;
        ImageReader next = ImageReader.newInstance(w, h, PixelFormat.RGBA_8888, 2);
        try {
            next.setOnImageAvailableListener(this::captureFrame, images);
            int dpi = getResources().getDisplayMetrics().densityDpi;
            if (display == null) {
                display = projection.createVirtualDisplay("TapSend screen", w, h, dpi,
                        DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, next.getSurface(), null, null);
                if (display == null) throw new IllegalStateException("Could not create the screen display.");
            } else {
                display.resize(w, h, dpi);
                display.setSurface(next.getSurface());
            }
            reader = next;
            latest.clear();
        } catch (RuntimeException error) {
            next.close();
            fail("Android could not resize screen sharing.");
        }
        if (old != null && reader == next) old.close();
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (Build.VERSION.SDK_INT < 34 && running && display != null) {
            Rect bounds = getSystemService(WindowManager.class).getMaximumWindowMetrics().getBounds();
            resizeCapture(bounds.width(), bounds.height());
        }
    }

    private void captureFrame(ImageReader source) {
        try (Image image = source.acquireLatestImage()) {
            long now = SystemClock.elapsedRealtime();
            if (!running || image == null || source != reader || now - lastFrame < MIN_FRAME_INTERVAL_MS) return;
            lastFrame = now;
            Image.Plane plane = image.getPlanes()[0];
            int w = image.getWidth(), h = image.getHeight(), stride = plane.getRowStride() / 4;
            if (plane.getPixelStride() != 4 || stride < w) return;
            packed = bitmap(packed, stride, h);
            ByteBuffer buffer = plane.getBuffer();
            if (buffer.remaining() < packed.getByteCount()) {
                if (padded == null || padded.capacity() != packed.getByteCount()) {
                    padded = ByteBuffer.allocate(packed.getByteCount());
                }
                padded.clear();
                padded.put(buffer).rewind();
                buffer = padded;
            }
            packed.copyPixelsFromBuffer(buffer);
            Bitmap result = packed;
            if (stride != w) {
                cropped = bitmap(cropped, w, h);
                Rect content = new Rect(0, 0, w, h);
                new Canvas(cropped).drawBitmap(packed, content, content, null);
                result = cropped;
            }
            encoded.reset();
            if (result.compress(Bitmap.CompressFormat.JPEG, 60, encoded)
                    && encoded.size() <= 512 * 1024 && running && source == reader) {
                // Slow links should catch up to the newest frame instead of growing a backlog.
                byte[] frame = encoded.toByteArray();
                if (!latest.offer(frame)) {
                    latest.poll();
                    latest.offer(frame);
                }
            }
        } catch (IllegalStateException ignored) {
            // The previous reader can close while a resize callback replaces it.
        }
    }

    private static Bitmap bitmap(Bitmap existing, int width, int height) {
        if (existing != null && existing.getWidth() == width && existing.getHeight() == height) return existing;
        if (existing != null) existing.recycle();
        return Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
    }

    private void fail(String detail) {
        Toast.makeText(this, detail, Toast.LENGTH_LONG).show();
        stopSelf();
    }

    @Override public void onDestroy() {
        active = running = false;
        latest.clear();
        if (arrow != null) arrow.close();
        TapSendClient phone = client;
        if (phone != null) phone.cancelTransfer();
        sender.shutdownNow();
        if (display != null) display.release();
        ImageReader source = reader;
        reader = null;
        if (source != null) source.close();
        if (projection != null) {
            if (callback != null) projection.unregisterCallback(callback);
            projection.stop();
        }
        if (images != null) images.post(() -> {
            if (packed != null) packed.recycle();
            if (cropped != null) cropped.recycle();
        });
        if (imageThread != null) imageThread.quitSafely();
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
    }
}
