package com.tapsend.android;

import android.os.Build;
import android.net.Network;
import android.util.Base64;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.Signature;
import java.security.cert.CertificateException;
import java.security.cert.X509Certificate;
import java.util.Arrays;
import java.util.UUID;

import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSocketFactory;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;

final class TapSendClient {
    interface Progress { void update(long done, long total); }
    interface InputOpener { InputStream open() throws Exception; }

    private final IdentityStore store;
    private final Network network;
    private volatile IdentityStore.Identity identity;
    private String base;
    private byte[] pin;
    private byte[] socketFactoryPin;
    private SSLSocketFactory socketFactory;
    private volatile String token;
    private volatile HttpsURLConnection activeTransfer;
    private volatile boolean cancelled;

    TapSendClient(IdentityStore store) { this(store, null); }

    TapSendClient(IdentityStore store, Network network) {
        this.store = store;
        this.network = network;
    }

    String host() { return base == null ? "" : base; }

    boolean hasIdentity() { return identity != null; }
    String identityId() { return identity == null ? "" : identity.deviceId; }

    void startScreen(String id, boolean redPen) throws Exception {
        json("POST", "/api/screen/start", new JSONObject().put("id", id).put("redPen", redPen), true);
    }

    String sendScreenFrame(String id, byte[] frame) throws Exception {
        for (int attempt = 0; attempt < 2; attempt++) {
            if (cancelled) throw new IOException("Screen sharing stopped.");
            HttpsURLConnection connection = open("POST", "/api/screen/frame/" + id, true);
            activeTransfer = connection;
            boolean reusable = false;
            try {
                connection.setInstanceFollowRedirects(false);
                connection.setConnectTimeout(2500);
                connection.setReadTimeout(2500);
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "image/jpeg");
                connection.setFixedLengthStreamingMode(frame.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(frame); }
                if (connection.getResponseCode() == 401 && attempt == 0) {
                    authenticate();
                    continue;
                }
                checkSuccess(connection);
                String arrow = connection.getHeaderField("X-TapSend-Arrow");
                connection.getInputStream().close(); // Return the empty successful response to the connection pool.
                reusable = true;
                return arrow;
            } finally {
                if (!reusable || cancelled) connection.disconnect();
                activeTransfer = null;
            }
        }
        return null;
    }

    void stopScreen(String id) {
        try {
            HttpsURLConnection connection = open("POST", "/api/screen/stop/" + id, true);
            try {
                connection.setInstanceFollowRedirects(false);
                connection.setConnectTimeout(1500);
                connection.setReadTimeout(1500);
                connection.getResponseCode();
            } finally { connection.disconnect(); }
        } catch (Exception ignored) { }
    }

    void sendPaste(String id, String kind, String label, File payload) throws Exception {
        for (int attempt = 0; attempt < 2; attempt++) {
            if (cancelled) throw new IOException("Paste Stack delivery stopped.");
            HttpsURLConnection connection = open("POST", "/api/paste-stack/" + id, true);
            activeTransfer = connection;
            try {
                connection.setInstanceFollowRedirects(false);
                connection.setRequestProperty("X-TapSend-Kind", kind);
                connection.setRequestProperty("X-TapSend-Label", Base64.encodeToString(
                        label.getBytes(StandardCharsets.UTF_8), Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING));
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(payload.length());
                try (OutputStream output = connection.getOutputStream()) {
                    java.nio.file.Files.copy(payload.toPath(), output);
                }
                if (connection.getResponseCode() == 401 && attempt == 0) {
                    authenticate();
                    continue;
                }
                checkSuccess(connection);
                return;
            } finally {
                connection.disconnect();
                activeTransfer = null;
            }
        }
    }

    void loadAndAuthenticate() throws Exception {
        identity = store.load();
        if (identity == null) return;
        base = identity.base;
        pin = identity.pin;
        authenticate();
    }

    static String defaultDeviceName() {
        String name = (Build.MANUFACTURER + " " + Build.MODEL).trim();
        return name.isEmpty() ? "Android phone" : name;
    }

    void pair(PairingLink link, String name) throws Exception {
        String previousBase = base;
        byte[] previousPin = pin;
        boolean saved = false;
        base = link.base;
        pin = link.certificatePin;
        try {
            KeyPair keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
            String id = UUID.randomUUID().toString();
            byte[] encoded = keys.getPublic().getEncoded();
            byte[] prefix = {0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00};
            if (encoded.length != 44 || !Arrays.equals(Arrays.copyOf(encoded, 12), prefix)) {
                throw new IllegalStateException("Unsupported Ed25519 key format on this phone.");
            }
            JSONObject body = new JSONObject()
                    .put("code", link.code).put("deviceID", id)
                    .put("name", name)
                    .put("publicKey", Base64.encodeToString(Arrays.copyOfRange(encoded, 12, 44),
                            Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING));
            json("POST", "/api/pair", body, false);
            IdentityStore.Identity next = new IdentityStore.Identity(base, pin, id, keys.getPrivate());
            store.save(next);
            identity = next;
            saved = true;
            authenticate();
        } catch (Exception error) {
            if (!saved) {
                base = previousBase;
                pin = previousPin;
            }
            throw error;
        }
    }

    void forget() {
        store.clear();
        identity = null;
        token = null;
        base = null;
        pin = null;
    }

    private synchronized void authenticate() throws Exception {
        if (identity == null) throw new IOException("Scan a pairing QR first.");
        JSONObject challenge = json("POST", "/api/challenge",
                new JSONObject().put("deviceID", identity.deviceId), false);
        Signature signer = Signature.getInstance("Ed25519");
        signer.initSign(identity.privateKey);
        signer.update(challenge.getString("challenge").getBytes(StandardCharsets.UTF_8));
        String signature = Base64.encodeToString(signer.sign(),
                Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
        JSONObject verified = json("POST", "/api/verify",
                new JSONObject().put("deviceID", identity.deviceId).put("signature", signature), false);
        token = verified.getString("token");
    }

    JSONArray files() throws Exception { return new JSONArray(jsonText("GET", "/api/files", null, true)); }
    JSONArray messages() throws Exception { return new JSONArray(jsonText("GET", "/api/messages", null, true)); }
    JSONArray snippets() throws Exception { return new JSONArray(jsonText("GET", "/api/scratchpad", null, true)); }
    void sendSnippet(String text) throws Exception {
        json("POST", "/api/scratchpad", new JSONObject().put("text", text), true);
    }
    JSONObject session() throws Exception { return new JSONObject(jsonText("GET", "/api/session", null, true)); }
    void answerOffer(String id, String state) throws Exception {
        if (!id.matches("[0-9a-f]{32}")) throw new IOException("Invalid offer ID.");
        json("POST", "/api/offers/" + id, new JSONObject().put("state", state), true);
    }
    boolean stopped() throws Exception { return session().getBoolean("stopped"); }
    void message(String text) throws Exception {
        json("POST", "/api/messages", new JSONObject().put("text", text), true);
    }
    void receipt(String id) { receipt(id, "received"); }
    void receipt(String id, String state) {
        try { json("POST", "/api/receipt", new JSONObject().put("id", id).put("state", state), true); }
        catch (Exception ignored) { }
    }

    boolean wasCancelled() { return cancelled; }

    File stageDownload(String id, long size, File cache, Progress progress) throws Exception {
        cancelled = false;
        HttpsURLConnection info = open("HEAD", "/api/files/" + id, true);
        String revision;
        try {
            activeTransfer = info;
            if (info.getResponseCode() == 401) {
                info.disconnect();
                authenticate();
                info = open("HEAD", "/api/files/" + id, true);
                activeTransfer = info;
            }
            checkSuccess(info);
            revision = info.getHeaderField("ETag");
            if (revision == null) return null; // Older servers still support a fresh download.
            if (info.getContentLengthLong() != size) throw new IOException("The shared file changed. Ask the computer to share it again.");
        } finally {
            info.disconnect();
            activeTransfer = null;
        }
        if (cancelled) throw new IOException("Download stopped.");
        File directory = new File(cache, "tapsend-downloads");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IOException("Could not create transfer storage.");
        String key = Base64.encodeToString(MessageDigest.getInstance("SHA-256")
                .digest(revision.getBytes(StandardCharsets.UTF_8)), Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
        File part = new File(directory, key + ".part");
        if (part.length() > size && !part.delete()) throw new IOException("Could not reset the partial download.");
        long offset = part.length();
        if (offset < size) {
            try (OutputStream destination = new FileOutputStream(part, true)) {
                downloadRange(id, size, offset, revision, destination, progress);
            }
        } else {
            progress.update(size, size);
        }
        return part;
    }

    void download(String id, long size, OutputStream destination, Progress progress) throws Exception {
        cancelled = false;
        downloadRange(id, size, 0, null, destination, progress);
    }

    private void downloadRange(String id, long size, long offset, String revision,
                               OutputStream destination, Progress progress) throws Exception {
        HttpsURLConnection connection = open("GET", "/api/files/" + id, true);
        activeTransfer = connection;
        try {
            setDownloadRange(connection, offset, revision);
            if (connection.getResponseCode() == 401) {
                connection.disconnect();
                authenticate();
                connection = open("GET", "/api/files/" + id, true);
                activeTransfer = connection;
                setDownloadRange(connection, offset, revision);
            }
            checkSuccess(connection);
            if (cancelled) throw new IOException("Download stopped.");
            if (offset > 0 && (connection.getResponseCode() != 206
                    || !("bytes " + offset + "-" + (size - 1) + "/" + size).equals(connection.getHeaderField("Content-Range")))) {
                throw new IOException("The computer could not resume this file. Ask it to share the file again.");
            }
            long done = offset;
            progress.update(done, size);
            byte[] buffer = new byte[256 * 1024];
            try (InputStream input = connection.getInputStream()) {
                int count;
                while ((count = input.read(buffer)) != -1) {
                    if (cancelled) throw new IOException("Download stopped.");
                    if (count > size - done) throw new IOException("The shared file changed during the download.");
                    destination.write(buffer, 0, count);
                    done += count;
                    progress.update(done, size);
                }
            }
            if (done != size) throw new IOException("Incomplete download. Save the file again to retry.");
            destination.flush();
        } catch (IOException error) {
            if (cancelled) throw new IOException("Download stopped.", error);
            throw error;
        } finally {
            connection.disconnect();
            activeTransfer = null;
        }
    }

    private static void setDownloadRange(HttpsURLConnection connection, long offset, String revision) {
        if (revision != null) connection.setRequestProperty("If-Match", revision);
        if (offset > 0) connection.setRequestProperty("Range", "bytes=" + offset + "-");
    }

    JSONObject folderDrop() throws Exception {
        stopped();
        HttpsURLConnection connection = open("GET", "/api/folder-drop", true);
        try {
            if (connection.getResponseCode() == 404) return new JSONObject();
            checkSuccess(connection);
            return new JSONObject(read(connection.getInputStream()));
        } finally { connection.disconnect(); }
    }

    JSONObject upload(String name, long size, String folderToken, InputOpener opener, Progress progress) throws Exception {
        cancelled = false;
        // Also refresh an expired token before opening a streaming request.
        int limit = session().optInt("uploadLimitGiB", 8);
        if (limit < 1 || limit > 64) limit = 8;
        if (size > ((long) limit << 30)) throw new IOException("The computer accepts uploads up to " + limit + " GiB.");
        for (int attempt = 0; attempt < 2; attempt++) {
            HttpsURLConnection connection = open("POST", "/api/upload", true);
            activeTransfer = connection;
            connection.setDoOutput(true);
            connection.setChunkedStreamingMode(256 * 1024);
            connection.setRequestProperty("X-File-Name", asciiName(name));
            if (!folderToken.isEmpty()) {
                connection.setRequestProperty("X-TapSend-Folder-Drop", folderToken);
                connection.setRequestProperty("X-TapSend-File-Name", Base64.encodeToString(
                        name.getBytes(StandardCharsets.UTF_8), Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING));
            }
            try {
                long done = 0;
                byte[] buffer = new byte[256 * 1024];
                try (InputStream input = opener.open(); OutputStream output = connection.getOutputStream()) {
                    int count;
                    while ((count = input.read(buffer)) != -1) {
                        if (cancelled) throw new IOException("Upload stopped.");
                        output.write(buffer, 0, count);
                        done += count;
                        progress.update(done, size);
                    }
                }
                if (connection.getResponseCode() == 401 && attempt == 0) {
                    authenticate();
                    continue;
                }
                checkSuccess(connection);
                return new JSONObject(read(connection.getInputStream()));
            } catch (IOException error) {
                if (cancelled) throw new IOException("Upload stopped.", error);
                throw error;
            } finally {
                connection.disconnect();
                activeTransfer = null;
            }
        }
        throw new IOException("Could not confirm the upload was saved.");
    }

    void cancelTransfer() {
        cancelled = true;
        HttpsURLConnection connection = activeTransfer;
        if (connection != null) connection.disconnect();
    }

    private JSONObject json(String method, String path, JSONObject body, boolean authenticated) throws Exception {
        return new JSONObject(jsonText(method, path, body, authenticated));
    }

    private String jsonText(String method, String path, JSONObject body, boolean authenticated) throws Exception {
        for (int attempt = 0; attempt < 2; attempt++) {
            HttpsURLConnection connection = open(method, path, authenticated);
            try {
                if (body != null) {
                    connection.setDoOutput(true);
                    connection.setRequestProperty("Content-Type", "application/json");
                    byte[] bytes = body.toString().getBytes(StandardCharsets.UTF_8);
                    connection.setFixedLengthStreamingMode(bytes.length);
                    try (OutputStream output = connection.getOutputStream()) { output.write(bytes); }
                }
                int status = connection.getResponseCode();
                if (status == 401 && authenticated && attempt == 0) {
                    authenticate();
                    continue;
                }
                checkSuccess(connection);
                return read(connection.getInputStream());
            } finally {
                connection.disconnect();
            }
        }
        throw new IOException("Authentication failed.");
    }

    private HttpsURLConnection open(String method, String path, boolean authenticated) throws Exception {
        if (base == null || pin == null) throw new IOException("Scan a pairing QR first.");
        URL url = new URL(base + path);
        HttpsURLConnection connection = (HttpsURLConnection) (network == null
                ? url.openConnection() : network.openConnection(url));
        connection.setSSLSocketFactory(pinnedSocketFactory());
        connection.setHostnameVerifier(HttpsURLConnection.getDefaultHostnameVerifier());
        connection.setRequestMethod(method);
        connection.setConnectTimeout(6000);
        connection.setReadTimeout(15000);
        connection.setUseCaches(false);
        if (authenticated) connection.setRequestProperty("Authorization", "Bearer " + token);
        return connection;
    }

    private synchronized SSLSocketFactory pinnedSocketFactory() throws Exception {
        byte[] expected = pin.clone();
        if (Arrays.equals(expected, socketFactoryPin)) return socketFactory;
        if (expected.length != 8 && expected.length != 16) throw new IOException("Invalid saved security key.");
        // QR pairing pins 128 bits; manual pairing pins 64 bits. Hostname verification
        // remains enabled above, and both pins identify this exact leaf certificate.
        X509TrustManager trust = new X509TrustManager() {
            @Override public void checkClientTrusted(X509Certificate[] chain, String authType) throws CertificateException {
                throw new CertificateException("Client certificates are not supported.");
            }
            @Override public void checkServerTrusted(X509Certificate[] chain, String authType) throws CertificateException {
                if (chain == null || chain.length == 0) throw new CertificateException("No server certificate.");
                chain[0].checkValidity();
                try {
                    byte[] digest = MessageDigest.getInstance("SHA-256").digest(chain[0].getEncoded());
                    if (!MessageDigest.isEqual(Arrays.copyOf(digest, expected.length), expected)) {
                        throw new CertificateException("Computer certificate changed. Recheck the security key or scan a new QR.");
                    }
                } catch (CertificateException error) { throw error; }
                catch (Exception error) { throw new CertificateException(error); }
            }
            @Override public X509Certificate[] getAcceptedIssuers() { return new X509Certificate[0]; }
        };
        SSLContext context = SSLContext.getInstance("TLS");
        context.init(null, new TrustManager[]{trust}, null);
        socketFactory = context.getSocketFactory();
        socketFactoryPin = expected;
        return socketFactory;
    }

    private static void checkSuccess(HttpsURLConnection connection) throws IOException {
        int status = connection.getResponseCode();
        if (status < 200 || status >= 300) {
            InputStream error = connection.getErrorStream();
            String detail = error == null ? "" : read(error).trim();
            throw new IOException(detail.isEmpty() ? "Computer returned HTTP " + status : detail);
        }
    }

    private static String read(InputStream input) throws IOException {
        try (InputStream stream = input; ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[4096];
            int count;
            while ((count = stream.read(buffer)) != -1) {
                if (output.size() + count > 1024 * 1024) throw new IOException("Response too large.");
                output.write(buffer, 0, count);
            }
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    private static String asciiName(String name) {
        String safe = name.replaceAll("[^\\x20-\\x7e]", "_").replace('/', '_').replace('\\', '_');
        return safe.isEmpty() ? "file" : safe;
    }
}
