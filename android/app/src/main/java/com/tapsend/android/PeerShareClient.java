package com.tapsend.android;

import org.json.JSONArray;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.InetAddress;
import java.net.Inet4Address;
import java.net.URI;
import java.net.URL;

final class PeerShareClient {
    private final String base;

    PeerShareClient(String raw) throws Exception {
        URI uri = new URI(raw.trim());
        if (!"http".equals(uri.getScheme()) || uri.getUserInfo() != null || uri.getQuery() != null
                || uri.getFragment() != null || uri.getPort() < 1 || uri.getPort() > 65535
                || uri.getPath() == null || !uri.getPath().matches("/s/[0-9a-f]{48}/?")) {
            throw new IllegalArgumentException("Enter a TapSend phone link starting with http://.");
        }
        if (uri.getHost() == null || !uri.getHost().matches("(?:[0-9]{1,3}\\.){3}[0-9]{1,3}"))
            throw new IllegalArgumentException("The phone link needs a local numeric Wi-Fi address.");
        InetAddress address = InetAddress.getByName(uri.getHost());
        if (!(address instanceof Inet4Address) || !address.isSiteLocalAddress()) {
            throw new IllegalArgumentException("The phone link must use a local Wi-Fi address.");
        }
        base = "http://" + address.getHostAddress() + ":" + uri.getPort() + uri.getPath()
                + (uri.getPath().endsWith("/") ? "" : "/");
    }

    String base() { return base; }

    JSONArray files() throws Exception {
        HttpURLConnection connection = open("manifest");
        try (InputStream input = connection.getInputStream()) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (output.size() + count > 1024 * 1024) throw new IOException("Phone file list is too large.");
                output.write(buffer, 0, count);
            }
            return new JSONArray(output.toString("UTF-8"));
        } finally { connection.disconnect(); }
    }

    long download(int id, long size, OutputStream output, Progress progress) throws Exception {
        if (id < 0 || id > 99999) throw new IOException("Invalid file number.");
        HttpURLConnection connection = open("file/" + id);
        try (InputStream input = connection.getInputStream()) {
            byte[] buffer = new byte[65536];
            int count;
            long done = 0;
            while ((count = input.read(buffer)) != -1) {
                output.write(buffer, 0, count);
                done += count;
                progress.update(done);
            }
            if (size >= 0 && done != size) throw new IOException("Download incomplete. Try saving again.");
            try { return Long.parseLong(connection.getHeaderField("X-TapSend-Transfer")); }
            catch (NumberFormatException ignored) { return 0; }
        } finally { connection.disconnect(); }
    }

    void confirmSaved(int id, long transfer) {
        if (transfer <= 0) return;
        HttpURLConnection connection = null;
        try {
            connection = (HttpURLConnection) new URL(base + "receipt/" + id + "/" + transfer).openConnection();
            connection.setConnectTimeout(3000);
            connection.setReadTimeout(3000);
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.getResponseCode();
        } catch (IOException ignored) {
            // The file is already saved even when the sender cannot be reached.
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private HttpURLConnection open(String path) throws Exception {
        HttpURLConnection connection = (HttpURLConnection) new URL(base + path).openConnection();
        connection.setConnectTimeout(10000);
        connection.setReadTimeout(30000);
        connection.setInstanceFollowRedirects(false);
        connection.setRequestProperty("Accept", "application/json, application/octet-stream");
        int status = connection.getResponseCode();
        if (status == 202) {
            connection.disconnect();
            throw new IOException("Waiting for sender approval. This page will update after approval.");
        }
        if (status == 403) {
            connection.disconnect();
            throw new IOException("The sender denied this phone.");
        }
        if (status != 200) {
            connection.disconnect();
            throw new IOException("Phone share returned HTTP " + status + ".");
        }
        return connection;
    }

    interface Progress { void update(long bytes); }
}
