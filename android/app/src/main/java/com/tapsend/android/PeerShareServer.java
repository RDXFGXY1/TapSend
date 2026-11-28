package com.tapsend.android;

import android.content.ContentResolver;
import android.net.Uri;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.NetworkInterface;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** A temporary, read-only share. Its random URL is the access credential. */
final class PeerShareServer implements AutoCloseable {
    static final class File {
        final Uri uri;
        final String name;
        final long size;
        File(Uri uri, String name, long size) {
            this.uri = uri;
            this.name = name;
            this.size = size;
        }
    }

    private final ContentResolver resolver;
    private final ServerSocket listener;
    private final ExecutorService connections = Executors.newFixedThreadPool(4);
    private final List<File> files = Collections.synchronizedList(new ArrayList<>());
    private final Set<String> joined = new LinkedHashSet<>();
    private final Set<String> pending = new LinkedHashSet<>();
    private final Set<String> denied = new LinkedHashSet<>();
    private final Map<Long, Transfer> transfers = new LinkedHashMap<>();
    private long nextTransfer;
    private final String token;
    private final String url;
    private volatile boolean closed;

    private enum TransferState {
        SENDING("Sending"), SENT("Sent · save unconfirmed"), SAVED("Saved by receiver"), FAILED("Send failed");

        final String label;
        TransferState(String label) { this.label = label; }
    }

    private record Transfer(int file, String name, String receiver, TransferState state) {
        Transfer withState(TransferState state) { return new Transfer(file, name, receiver, state); }
    }

    PeerShareServer(ContentResolver resolver) throws IOException {
        this.resolver = resolver;
        byte[] secret = new byte[24];
        new SecureRandom().nextBytes(secret);
        StringBuilder code = new StringBuilder();
        for (byte b : secret) code.append(String.format(Locale.US, "%02x", b & 255));
        token = code.toString();
        InetAddress address = localAddress();
        listener = new ServerSocket(0, 16, address);
        url = "http://" + address.getHostAddress() + ":" + listener.getLocalPort() + "/s/" + token + "/";
        Thread acceptor = new Thread(this::accept, "tapsend-peer-accept");
        acceptor.setDaemon(true);
        acceptor.start();
    }

    String url() { return url; }

    void add(File file) { files.add(file); }

    int count() { return files.size(); }

    synchronized List<String> pending() { return new ArrayList<>(pending); }

    synchronized int joinedCount() { return joined.size(); }

    synchronized String transferSummary() {
        if (transfers.isEmpty()) return "No files sent yet";
        List<String> lines = new ArrayList<>();
        for (Transfer transfer : transfers.values()) {
            lines.add(transfer.state.label + " · " + transfer.name + "\n" + transfer.receiver);
        }
        Collections.reverse(lines);
        return String.join("\n\n", lines.subList(0, Math.min(8, lines.size())));
    }

    private synchronized long startTransfer(int id, File file, String receiver) {
        long transfer = ++nextTransfer;
        transfers.put(transfer, new Transfer(id, file.name, receiver, TransferState.SENDING));
        if (transfers.size() > 50) transfers.remove(transfers.keySet().iterator().next());
        return transfer;
    }

    private synchronized void finishTransfer(long id, TransferState state) {
        Transfer transfer = transfers.get(id);
        if (transfer != null && transfer.state == TransferState.SENDING) {
            transfers.put(id, transfer.withState(state));
        }
    }

    private synchronized boolean confirmSaved(long id, int file, String receiver) {
        Transfer transfer = transfers.get(id);
        if (transfer == null || transfer.file != file || !transfer.receiver.equals(receiver)
                || transfer.state == TransferState.FAILED) return false;
        transfers.put(id, transfer.withState(TransferState.SAVED));
        return true;
    }

    synchronized void approve(String address) {
        if (pending.remove(address)) joined.add(address);
    }

    synchronized void deny(String address) {
        if (pending.remove(address)) denied.add(address);
    }

    private synchronized int join(String address) {
        if (denied.contains(address)) return 2;
        if (joined.contains(address)) return 0;
        if (pending.contains(address)) return 1;
        if (joined.size() < 2) {
            joined.add(address);
            return 0;
        }
        pending.add(address);
        return 1;
    }

    private static InetAddress localAddress() throws IOException {
        for (NetworkInterface iface : Collections.list(NetworkInterface.getNetworkInterfaces())) {
            if (!iface.isUp() || iface.isLoopback() || iface.isVirtual()) continue;
            String name = iface.getName();
            if (!(name.startsWith("wlan") || name.startsWith("wifi") || name.startsWith("ap")
                    || name.startsWith("eth") || name.startsWith("rndis") || name.startsWith("usb"))) continue;
            for (InetAddress address : Collections.list(iface.getInetAddresses())) {
                if (!(address instanceof Inet4Address) || address.isLoopbackAddress() || address.isLinkLocalAddress()) continue;
                if (!address.isSiteLocalAddress()) continue;
                return address;
            }
        }
        throw new IOException("Connect this phone to Wi-Fi or a hotspot first.");
    }

    private void accept() {
        while (!closed) {
            try {
                Socket socket = listener.accept();
                socket.setSoTimeout(15000);
                connections.execute(() -> serve(socket));
            } catch (IOException error) {
                if (!closed) break;
            }
        }
    }

    private void serve(Socket socket) {
        try (Socket client = socket;
             InputStream input = new BufferedInputStream(client.getInputStream());
             OutputStream output = new BufferedOutputStream(client.getOutputStream())) {
            String first = line(input);
            if (first == null || first.length() > 4096) return;
            String[] request = first.split(" ");
            if (request.length < 2) return;
            int headerBytes = 0;
            String header;
            while ((header = line(input)) != null && !header.isEmpty()) {
                headerBytes += header.length();
                if (headerBytes > 8192) return;
            }
            String path = request[1].split("\\?", 2)[0];
            String prefix = "/s/" + token + "/";
            if (!path.startsWith(prefix)) {
                reply(output, 404, "text/plain; charset=utf-8", "Not found".getBytes(StandardCharsets.UTF_8));
                return;
            }
            String resource = path.substring(prefix.length());
            boolean get = request[0].equals("GET");
            boolean receipt = request[0].equals("POST") && resource.matches("receipt/[0-9]{1,5}/[0-9]{1,18}");
            if (!get && !receipt) {
                reply(output, 405, "text/plain", new byte[0]);
                return;
            }
            String receiver = client.getInetAddress().getHostAddress();
            int admission = join(receiver);
            if (admission != 0) {
                if (resource.isEmpty() && admission == 1) {
                    String waiting = "<!doctype html><meta name=viewport content='width=device-width,initial-scale=1'>"
                            + "<meta http-equiv=refresh content=3><title>TapSend approval</title>"
                            + "<style>body{background:#000;color:#fff;font:18px monospace;max-width:600px;margin:48px auto;padding:20px}"
                            + "</style><h1>Waiting for approval</h1><p>Ask the sender to approve this phone. This page will refresh.</p>";
                    reply(output, 200, "text/html; charset=utf-8", waiting.getBytes(StandardCharsets.UTF_8));
                } else {
                    String reason = admission == 1 ? "Waiting for sender approval" : "Sender denied this phone";
                    reply(output, admission == 1 ? 202 : 403, "text/plain; charset=utf-8", reason.getBytes(StandardCharsets.UTF_8));
                }
                return;
            }
            if (receipt) {
                String[] parts = resource.split("/");
                boolean saved = confirmSaved(Long.parseLong(parts[2]), Integer.parseInt(parts[1]), receiver);
                reply(output, saved ? 200 : 404, "text/plain", new byte[0]);
            } else if (resource.isEmpty()) {
                reply(output, 200, "text/html; charset=utf-8", page().getBytes(StandardCharsets.UTF_8));
            } else if (resource.equals("manifest")) {
                JSONArray items = new JSONArray();
                synchronized (files) {
                    for (int i = 0; i < files.size(); i++) {
                        File file = files.get(i);
                        JSONObject item = new JSONObject();
                        item.put("id", i);
                        item.put("name", file.name);
                        item.put("size", file.size);
                        items.put(item);
                    }
                }
                reply(output, 200, "application/json; charset=utf-8", items.toString().getBytes(StandardCharsets.UTF_8));
            } else if (resource.matches("file/[0-9]{1,5}")) {
                int index = Integer.parseInt(resource.substring(5));
                File file;
                synchronized (files) { file = index < files.size() ? files.get(index) : null; }
                if (file == null) { reply(output, 404, "text/plain", new byte[0]); return; }
                long transfer = startTransfer(index, file, receiver);
                try (InputStream body = resolver.openInputStream(file.uri)) {
                    if (body == null) throw new IOException("File is no longer readable");
                    String safeName = file.name.replaceAll("[^A-Za-z0-9._ -]", "_").replace('"', '_');
                    String headers = "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n"
                            + "Content-Disposition: attachment; filename=\"" + safeName + "\"\r\n"
                            + "X-TapSend-Transfer: " + transfer + "\r\n"
                            + (file.size >= 0 ? "Content-Length: " + file.size + "\r\n" : "Transfer-Encoding: chunked\r\n")
                            + "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
                    output.write(headers.getBytes(StandardCharsets.US_ASCII));
                    byte[] buffer = new byte[65536];
                    int read;
                    long total = 0;
                    while ((read = body.read(buffer)) != -1) {
                        if (read == 0) continue;
                        total += read;
                        if (file.size >= 0 && total > file.size) throw new IOException("File size changed");
                        if (file.size < 0) output.write((Integer.toHexString(read) + "\r\n").getBytes(StandardCharsets.US_ASCII));
                        output.write(buffer, 0, read);
                        if (file.size < 0) output.write("\r\n".getBytes(StandardCharsets.US_ASCII));
                    }
                    if (file.size >= 0 && total != file.size) throw new IOException("File size changed");
                    if (file.size < 0) output.write("0\r\n\r\n".getBytes(StandardCharsets.US_ASCII));
                    output.flush();
                    finishTransfer(transfer, TransferState.SENT);
                } catch (Exception error) {
                    finishTransfer(transfer, TransferState.FAILED);
                    throw error;
                }
            } else {
                reply(output, 404, "text/plain; charset=utf-8", "Not found".getBytes(StandardCharsets.UTF_8));
            }
        } catch (Exception ignored) {
            // A recipient may leave during a download; the session remains available.
        }
    }

    private String page() {
        StringBuilder html = new StringBuilder("<!doctype html><meta name=viewport content='width=device-width,initial-scale=1'>"
                + "<meta name=referrer content=no-referrer><title>TapSend</title><style>body{background:#000;color:#fff;"
                + "font:16px monospace;max-width:680px;margin:30px auto;padding:18px}header{font-size:28px;border-bottom:2px solid #fff;"
                + "padding-bottom:16px}a{color:#fff;display:block;border:1px solid #fff;padding:18px;margin:12px 0;"
                + "overflow-wrap:anywhere}small{display:block;margin-top:8px}p{line-height:1.5}</style><header>tapsend / receive</header>"
                + "<p>Choose a file to download from the nearby phone.</p>");
        synchronized (files) {
            if (files.isEmpty()) html.append("<p>No files are being shared yet. Refresh this page after the sender adds one.</p>");
            for (int i = 0; i < files.size(); i++) {
                File file = files.get(i);
                html.append("<a href='file/").append(i).append("'>↓ ").append(escape(file.name));
                if (file.size >= 0) html.append("<small>").append(String.format(Locale.US, "%.1f MiB", file.size / 1048576.0)).append("</small>");
                html.append("</a>");
            }
        }
        return html.toString();
    }

    private static String escape(String value) {
        return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("'", "&#39;").replace("\"", "&quot;");
    }

    private static String line(InputStream input) throws IOException {
        StringBuilder result = new StringBuilder();
        int value;
        while ((value = input.read()) != -1) {
            if (value == '\n') return result.toString();
            if (value != '\r') result.append((char) value);
            if (result.length() > 8192) throw new IOException("Request too long");
        }
        return null;
    }

    private static void reply(OutputStream output, int status, String type, byte[] body) throws IOException {
        output.write(("HTTP/1.1 " + status + (status == 200 ? " OK" : " Error") + "\r\nContent-Type: " + type
                + "\r\nContent-Length: " + body.length + "\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff"
                + "\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nReferrer-Policy: no-referrer"
                + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
        output.write(body);
        output.flush();
    }

    @Override public void close() {
        closed = true;
        try { listener.close(); } catch (IOException ignored) { }
        connections.shutdownNow();
        files.clear();
        synchronized (this) { joined.clear(); pending.clear(); denied.clear(); }
    }
}
