package com.tapsend.android;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.provider.OpenableColumns;

import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.UUID;

final class PasteOutbox {
    private static final int IMAGE_LIMIT = 16 * 1024 * 1024;
    private static final int TEXT_LIMIT = 16 * 1024;
    private static final int PIXEL_LIMIT = 12_000_000;

    private PasteOutbox() {}

    static File root(Context context) {
        return new File(context.getFilesDir(), "paste-outbox");
    }

    static File[] batches(Context context, String device) {
        return sorted(root(context).listFiles(file -> file.isDirectory() && file.getName().endsWith("-" + device)));
    }

    static File[] entries(File batch) {
        return sorted(batch.listFiles(file -> file.isFile() && file.getName().endsWith(".json")));
    }

    private static File[] sorted(File[] files) {
        if (files == null) return new File[0];
        Arrays.sort(files, Comparator.comparing(File::getName));
        return files;
    }

    static synchronized int enqueue(Context context, List<Uri> images, List<String> texts) throws Exception {
        IdentityStore.Identity identity = new IdentityStore(context).load();
        if (identity == null) throw new IOException("Pair a computer before adding to Paste Stack.");
        int count = images.size() + texts.size();
        if (count < 1 || count > 16) throw new IOException("Choose 1–16 screenshots or text items.");
        File root = root(context);
        if (!root.isDirectory() && !root.mkdirs()) throw new IOException("Could not create the Paste Stack queue.");
        int queued = 0;
        File[] existing = root.listFiles(File::isDirectory);
        if (existing != null) {
            for (File batch : existing) {
                if (batch.getName().startsWith(".staging-")) deleteBatch(batch);
                else queued += entries(batch).length;
            }
        }
        if (queued + count > 64) throw new IOException("Paste Stack is full. Connect to your computer to send existing items.");
        String batchName = System.currentTimeMillis() + "-" + UUID.randomUUID();
        File staging = new File(root, ".staging-" + batchName);
        if (!staging.mkdir()) throw new IOException("Could not stage Paste Stack items.");
        try {
            int position = 0;
            for (Uri uri : images) {
                String label = displayName(context.getContentResolver(), uri);
                stageImage(context.getContentResolver(), uri, payload(staging, position));
                writeEntry(staging, position++, "image", label);
            }
            for (String text : texts) {
                byte[] body = text.getBytes(StandardCharsets.UTF_8);
                if (text.trim().isEmpty() || text.indexOf(0) >= 0 || body.length > TEXT_LIMIT) {
                    throw new IOException("Paste Stack text must be between 1 and 16 KiB.");
                }
                Files.write(payload(staging, position).toPath(), body);
                writeEntry(staging, position++, "text", "Text");
            }
            File published = new File(root, batchName + "-" + identity.deviceId);
            if (!staging.renameTo(published)) throw new IOException("Could not save the offline Paste Stack queue.");
        } finally {
            if (staging.exists()) deleteBatch(staging);
        }
        return count;
    }

    private static String displayName(ContentResolver resolver, Uri uri) {
        try (Cursor cursor = resolver.query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null && !name.isBlank()) return name.substring(0, Math.min(120, name.length()));
            }
        } catch (RuntimeException ignored) {}
        return "Screenshot";
    }

    private static void stageImage(ContentResolver resolver, Uri uri, File destination) throws IOException {
        try (InputStream input = resolver.openInputStream(uri); FileOutputStream output = new FileOutputStream(destination)) {
            if (input == null) throw new IOException("TapSend could not open a shared screenshot.");
            byte[] buffer = new byte[65536];
            int total = 0, count;
            while ((count = input.read(buffer)) != -1) {
                total += count;
                if (total > IMAGE_LIMIT) throw new IOException("Each screenshot can be up to 16 MiB.");
                output.write(buffer, 0, count);
            }
        }
        BitmapFactory.Options info = new BitmapFactory.Options();
        info.inJustDecodeBounds = true;
        BitmapFactory.decodeFile(destination.getPath(), info);
        if (info.outWidth <= 0 || info.outHeight <= 0 || info.outWidth > PIXEL_LIMIT / info.outHeight) {
            throw new IOException("Choose screenshots or images up to 12 megapixels.");
        }
        if ("image/png".equals(info.outMimeType) || "image/jpeg".equals(info.outMimeType)) return;
        Bitmap bitmap = BitmapFactory.decodeFile(destination.getPath());
        if (bitmap == null) throw new IOException("TapSend could not read this image.");
        try (FileOutputStream output = new FileOutputStream(destination)) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new IOException("Could not prepare the screenshot.");
        } finally {
            bitmap.recycle();
        }
        if (destination.length() > IMAGE_LIMIT) throw new IOException("The prepared screenshot exceeds 16 MiB.");
    }

    private static File payload(File batch, int position) {
        return new File(batch, String.format(Locale.ROOT, "%02d.clip", position));
    }

    static File payload(File entry) {
        return new File(entry.getParentFile(), entry.getName().replace(".json", ".clip"));
    }

    private static void writeEntry(File batch, int position, String kind, String label) throws Exception {
        JSONObject entry = new JSONObject().put("id", UUID.randomUUID().toString().replace("-", ""))
                .put("kind", kind).put("label", label);
        File metadata = new File(batch, String.format(Locale.ROOT, "%02d.json", position));
        Files.write(metadata.toPath(), entry.toString().getBytes(StandardCharsets.UTF_8));
    }

    static void deleteBatch(File batch) {
        File[] files = batch.listFiles();
        if (files != null) for (File file : files) file.delete();
        batch.delete();
    }
}
