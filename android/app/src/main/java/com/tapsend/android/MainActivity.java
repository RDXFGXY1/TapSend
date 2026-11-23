package com.tapsend.android;

import android.app.Activity;
import android.content.ClipData;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.Bitmap;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.os.Environment;
import android.os.Handler;
import android.os.Looper;
import android.provider.MediaStore;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.WindowInsets;
import android.widget.Button;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import com.google.mlkit.vision.codescanner.GmsBarcodeScanning;
import com.google.zxing.BarcodeFormat;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

public final class MainActivity extends Activity {
    private static final int PICK_FILE = 7;
    private static final int PICK_PEER_FILE = 8;
    private static final int TAB_SEND = 0;
    private static final int TAB_RECEIVE = 1;
    private static final int TAB_CONNECTION = 2;
    private static final int TAB_MESSAGES = 3;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final ExecutorService peerWorker = Executors.newSingleThreadExecutor();
    private final AtomicBoolean peerPollBusy = new AtomicBoolean();
    private final Handler handler = new Handler(Looper.getMainLooper());
    private TapSendClient client;
    private EditText linkInput;
    private EditText addressInput;
    private EditText codeInput;
    private EditText keyInput;
    private EditText messageInput;
    private TextView status;
    private TextView connection;
    private TextView session;
    private LinearLayout fileList;
    private LinearLayout thread;
    private LinearLayout uploadHistory;
    private LinearLayout peerFileList;
    private EditText peerLinkInput;
    private TextView peerShareLink;
    private TextView peerShareCount;
    private TextView peerJoinSummary;
    private LinearLayout peerJoinRequests;
    private String lastPeerJoins = "";
    private ImageView peerShareQr;
    private volatile PeerShareServer peerServer;
    private volatile PeerShareClient peerClient;
    private volatile String lastPeerCatalog = "";
    private final LinearLayout[] pages = new LinearLayout[4];
    private final Button[] tabButtons = new Button[4];
    private ScrollView contentScroll;
    private int activeTab = -1;
    private Button cancelButton;
    private volatile long latestMessageId = -1;
    private volatile String lastCatalog = "";
    private volatile boolean destroyed;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        client = new TapSendClient(new IdentityStore(this));
        buildUi();
        work("CONNECTING", () -> {
            client.loadAndAuthenticate();
            if (client.hasIdentity()) {
                showConnected();
                refreshFiles();
            } else {
                postStatus("Ready. Share with a nearby phone, or pair a computer in Connect.");
            }
        });
        handler.postDelayed(this::poll, 2500);
        handler.postDelayed(this::pollNearby, 1500);
    }

    @Override protected void onDestroy() {
        destroyed = true;
        client.cancelTransfer();
        if (peerServer != null) peerServer.close();
        worker.shutdownNow();
        peerWorker.shutdownNow();
        super.onDestroy();
    }

    private void buildUi() {
        getWindow().setStatusBarColor(Color.BLACK);
        getWindow().setNavigationBarColor(Color.BLACK);
        LinearLayout screen = new LinearLayout(this);
        screen.setOrientation(LinearLayout.VERTICAL);
        screen.setBackgroundColor(Color.BLACK);
        screen.setPadding(dp(16), dp(12), dp(16), dp(12));
        screen.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets bars = insets.getInsets(WindowInsets.Type.systemBars());
            view.setPadding(dp(16), dp(12) + bars.top, dp(16), dp(12) + bars.bottom);
            return insets;
        });
        setContentView(screen);

        LinearLayout masthead = new LinearLayout(this);
        masthead.setOrientation(LinearLayout.HORIZONTAL);
        masthead.setGravity(android.view.Gravity.CENTER_VERTICAL);
        screen.addView(masthead);
        masthead.addView(text("tapsend", 26, true));
        View rule = new View(this);
        rule.setBackgroundColor(Color.WHITE);
        LinearLayout.LayoutParams ruleParams = new LinearLayout.LayoutParams(0, dp(2), 1);
        ruleParams.leftMargin = dp(14);
        masthead.addView(rule, ruleParams);

        LinearLayout tabs = new LinearLayout(this);
        tabs.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams tabsParams = new LinearLayout.LayoutParams(-1, dp(52));
        tabsParams.topMargin = dp(14);
        screen.addView(tabs, tabsParams);
        String[] names = {"SEND", "RECEIVE", "CONNECT", "MESSAGES"};
        for (int i = 0; i < names.length; i++) {
            final int tab = i;
            Button button = button(names[i], false, view -> selectTab(tab));
            button.setTextSize(10);
            button.setSingleLine(true);
            button.setMinWidth(0);
            button.setPadding(0, 0, 0, 0);
            button.setContentDescription(names[i] + " tab");
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, -1, 1);
            if (i > 0) params.leftMargin = dp(5);
            tabs.addView(button, params);
            tabButtons[i] = button;
        }

        contentScroll = new ScrollView(this);
        contentScroll.setFillViewport(true);
        contentScroll.setBackgroundColor(Color.BLACK);
        LinearLayout.LayoutParams scrollParams = new LinearLayout.LayoutParams(-1, 0, 1);
        scrollParams.topMargin = dp(8);
        screen.addView(contentScroll, scrollParams);
        LinearLayout content = new LinearLayout(this);
        content.setOrientation(LinearLayout.VERTICAL);
        contentScroll.addView(content);
        pages[TAB_SEND] = buildSendPage();
        pages[TAB_RECEIVE] = buildReceivePage();
        pages[TAB_CONNECTION] = buildConnectionPage();
        pages[TAB_MESSAGES] = buildMessagesPage();
        for (LinearLayout page : pages) content.addView(page);

        LinearLayout statusPanel = panel(screen, "STATUS / LOCAL NETWORK", 0);
        status = text("Preparing connection…", 12, false);
        status.setContentDescription("Connection status");
        statusPanel.addView(status);
        cancelButton = button("[ STOP ACTIVE TRANSFER ]", false, view -> client.cancelTransfer());
        cancelButton.setVisibility(View.GONE);
        statusPanel.addView(cancelButton);
        selectTab(TAB_SEND);
    }

    private LinearLayout buildSendPage() {
        LinearLayout page = page();
        buildNearbySendPanel(page);
        divider(page);
        LinearLayout main = panel(page, "01 / SEND TO COMPUTER", 230);
        main.addView(text("CHOOSE FILES FROM THIS PHONE", 15, true));
        main.addView(text("Uploads go directly to the computer's TapSend inbox.", 12, false));
        uploadHistory = new LinearLayout(this);
        uploadHistory.setOrientation(LinearLayout.VERTICAL);
        uploadHistory.addView(text("NO UPLOADS THIS SESSION", 12, false));
        main.addView(uploadHistory);
        LinearLayout actions = actionRow(page);
        rowButton(actions, "[ CHOOSE FILES ]", true, view -> chooseFiles());
        rowButton(actions, "[ MESSAGES ]", false, view -> selectTab(TAB_MESSAGES));
        divider(page);
        LinearLayout details = panel(page, "DESTINATION", 140);
        details.addView(text("COMPUTER / TAPSEND INBOX", 15, true));
        details.addView(text("Choose several files in the picker to send them together. The computer stays paired for later transfers.", 12, false));
        return page;
    }

    private void buildNearbySendPanel(LinearLayout page) {
        LinearLayout nearby = panel(page, "SEND TO A NEARBY PHONE", 220);
        nearby.addView(text("The other phone opens your link in a browser. TapSend is not needed there.", 12, false));
        peerShareLink = text("SHARING OFF", 12, true);
        peerShareLink.setTextIsSelectable(true);
        nearby.addView(peerShareLink);
        peerShareQr = new ImageView(this);
        peerShareQr.setContentDescription("Phone share QR code");
        peerShareQr.setVisibility(View.GONE);
        LinearLayout.LayoutParams qrParams = new LinearLayout.LayoutParams(dp(220), dp(220));
        qrParams.topMargin = dp(12);
        nearby.addView(peerShareQr, qrParams);
        peerShareCount = text("0 FILES SHARED", 12, false);
        nearby.addView(peerShareCount);
        nearby.addView(button("[ START PHONE SHARE ]", true, view -> startPeerShare()));
        nearby.addView(button("[ ADD FILES TO PHONE SHARE ]", false, view -> choosePeerFiles()));
        nearby.addView(button("[ COPY LINK ]", false, view -> copyPeerLink()));
        nearby.addView(button("[ STOP PHONE SHARE ]", false, view -> stopPeerShare()));
        peerJoinSummary = text("0 RECEIVERS", 12, true);
        nearby.addView(peerJoinSummary);
        peerJoinRequests = new LinearLayout(this);
        peerJoinRequests.setOrientation(LinearLayout.VERTICAL);
        nearby.addView(peerJoinRequests);
        nearby.addView(text("The first two phones join directly. Approve each additional phone below.", 11, false));
        nearby.addView(text("Keep this screen open while the other phone downloads. Use a trusted Wi-Fi or hotspot.", 11, false));
    }

    private LinearLayout buildReceivePage() {
        LinearLayout page = page();
        buildNearbyReceivePanel(page);
        divider(page);
        LinearLayout main = panel(page, "02 / RECEIVE FROM COMPUTER", 280);
        session = text("WAITING FOR CONNECTION", 12, true);
        main.addView(session);
        fileList = new LinearLayout(this);
        fileList.setOrientation(LinearLayout.VERTICAL);
        fileList.addView(text("PAIRED FILES WILL APPEAR HERE", 12, false));
        main.addView(fileList);
        LinearLayout actions = actionRow(page);
        rowButton(actions, "[ REFRESH ]", true, view -> work("REFRESHING", this::refreshFiles));
        rowButton(actions, "[ CONNECT ]", false, view -> selectTab(TAB_CONNECTION));
        divider(page);
        LinearLayout details = panel(page, "SAVED FILES", 150);
        details.addView(text("DOWNLOADS / TAPSEND", 15, true));
        details.addView(text("Tap a shared file above to save it. New files added on the computer appear automatically without a new QR scan.", 12, false));
        return page;
    }

    private void buildNearbyReceivePanel(LinearLayout page) {
        LinearLayout nearby = panel(page, "RECEIVE FROM A NEARBY PHONE", 220);
        nearby.addView(text("Paste or scan the link shown on the sending phone.", 12, false));
        peerLinkInput = input("http://192.168.x.x:port/s/...");
        peerLinkInput.setText(getPreferences(0).getString("peer_link", ""));
        nearby.addView(peerLinkInput);
        LinearLayout peerActions = actionRow(nearby);
        rowButton(peerActions, "[ OPEN LINK ]", true, view -> openPeerLink());
        rowButton(peerActions, "[ SCAN QR ]", false, view -> scanPeerLink());
        peerFileList = new LinearLayout(this);
        peerFileList.setOrientation(LinearLayout.VERTICAL);
        peerFileList.addView(text("NO PHONE CONNECTED", 12, false));
        nearby.addView(peerFileList);
    }

    private LinearLayout buildConnectionPage() {
        LinearLayout page = page();
        LinearLayout main = panel(page, "03 / CONNECTION", 230);
        connection = text("OFFLINE", 16, true);
        main.addView(connection);
        main.addView(text("This phone and the computer must be on the same local network.", 12, false));
        main.addView(text("A saved phone reconnects without scanning again.", 12, false));
        LinearLayout actions = actionRow(page);
        rowButton(actions, "[ SAVED PHONE ]", true, view -> work("CONNECTING", () -> {
            client.loadAndAuthenticate();
            if (!client.hasIdentity()) throw new IllegalStateException("No saved phone. Scan QR or enter pairing details.");
            showConnected();
            refreshFiles();
        }));
        rowButton(actions, "[ SCAN QR ]", false, view -> scan());
        divider(page);
        LinearLayout manual = panel(page, "PAIR MANUALLY", 270);
        manual.addView(text("TYPE THE DETAILS SHOWN IN THE COMPUTER TUI", 12, true));
        addressInput = input("Computer address (192.168.2.52:8443)");
        codeInput = input("8-digit code (1234-5678)");
        keyInput = input("Security key (XXXX-XXXX-XXXXX)");
        codeInput.setInputType(android.text.InputType.TYPE_CLASS_NUMBER);
        keyInput.setInputType(android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS
                | android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD);
        manual.addView(text("Dashes are optional. The code expires after five minutes.", 11, false));
        manual.addView(addressInput);
        manual.addView(codeInput);
        manual.addView(keyInput);
        manual.addView(button("[ PAIR WITH CODE ]", true, view -> pairManual()));
        linkInput = input("Paste the pairing link from the terminal");
        manual.addView(linkInput);
        manual.addView(button("[ PAIR FROM LINK ]", false, view -> pair(linkInput.getText().toString())));
        manual.addView(button("[ FORGET LOCAL PAIRING ]", false, view -> {
            client.cancelTransfer();
            work("FORGETTING", () -> {
                client.forget();
                runOnUiThread(() -> {
                    fileList.removeAllViews();
                    thread.removeAllViews();
                    latestMessageId = -1;
                    lastCatalog = "";
                    connection.setText("OFFLINE");
                    session.setText("WAITING FOR CONNECTION");
                    status.setText("Phone identity removed. Scan a fresh QR to pair again.");
                    selectTab(TAB_CONNECTION);
                });
            });
        }));
        return page;
    }

    private LinearLayout buildMessagesPage() {
        LinearLayout page = page();
        LinearLayout main = panel(page, "04 / MESSAGES", 260);
        thread = new LinearLayout(this);
        thread.setOrientation(LinearLayout.VERTICAL);
        thread.addView(text("NO MESSAGES YET", 12, false));
        main.addView(thread);
        divider(page);
        LinearLayout compose = panel(page, "REPLY TO COMPUTER", 170);
        messageInput = input("Type a message");
        compose.addView(messageInput);
        compose.addView(button("[ SEND MESSAGE ]", true, view -> sendMessage()));
        return page;
    }

    private LinearLayout page() {
        LinearLayout page = new LinearLayout(this);
        page.setOrientation(LinearLayout.VERTICAL);
        page.setPadding(0, 0, 0, dp(18));
        return page;
    }

    private LinearLayout panel(LinearLayout parent, String title, int minimumHeight) {
        LinearLayout panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setPadding(dp(17), dp(16), dp(17), dp(18));
        panel.setBackground(border(dp(22)));
        if (minimumHeight > 0) panel.setMinimumHeight(dp(minimumHeight));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(14);
        parent.addView(panel, params);
        panel.addView(text(title, 13, true));
        return panel;
    }

    private LinearLayout actionRow(LinearLayout parent) {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, dp(54));
        params.topMargin = dp(12);
        parent.addView(row, params);
        return row;
    }

    private void rowButton(LinearLayout row, String label, boolean filled, View.OnClickListener click) {
        Button button = button(label, filled, click);
        button.setTextSize(10);
        button.setSingleLine(true);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, -1, 1);
        if (row.getChildCount() > 0) params.leftMargin = dp(10);
        row.addView(button, params);
    }

    private void selectTab(int tab) {
        if (tab < 0 || tab >= pages.length) return;
        activeTab = tab;
        for (int i = 0; i < pages.length; i++) {
            pages[i].setVisibility(i == tab ? View.VISIBLE : View.GONE);
            tabButtons[i].setTextColor(i == tab ? Color.BLACK : Color.WHITE);
            GradientDrawable background = border(dp(12));
            if (i == tab) background.setColor(Color.WHITE);
            tabButtons[i].setBackground(background);
        }
        if (contentScroll != null) contentScroll.post(() -> contentScroll.scrollTo(0, 0));
    }

    private void scan() {
        GmsBarcodeScanning.getClient(this).startScan()
                .addOnSuccessListener(code -> {
                    String value = code.getRawValue();
                    if (value != null) {
                        linkInput.setText(value);
                        pair(value);
                    }
                })
                .addOnFailureListener(error -> postStatus("Scanner unavailable: " + detail(error) + ". Paste the link instead."));
    }

    private void pair(String raw) {
        final PairingLink link;
        try { link = PairingLink.parse(raw); }
        catch (Exception error) { postStatus(detail(error)); return; }
        pair(link);
    }

    private void pairManual() {
        final PairingLink link;
        try { link = PairingLink.fromManual(addressInput.getText().toString(),
                codeInput.getText().toString(), keyInput.getText().toString()); }
        catch (Exception error) { postStatus(detail(error)); return; }
        pair(link);
    }

    private void pair(PairingLink link) {
        work("PAIRING", () -> {
            client.pair(link);
            latestMessageId = -1;
            lastCatalog = "";
            runOnUiThread(() -> thread.removeAllViews());
            showConnected();
            refreshFiles();
            runOnUiThread(() -> {
                linkInput.setText("");
                codeInput.setText("");
                keyInput.setText("");
            });
        });
    }

    private void showConnected() {
        runOnUiThread(() -> {
            connection.setText("● CONNECTED  " + client.host());
            status.setText("Authenticated phone. Ready to transfer.");
            if (activeTab == TAB_CONNECTION) selectTab(TAB_RECEIVE);
        });
    }

    private void refreshFiles() throws Exception {
        if (!client.hasIdentity()) throw new IllegalStateException("Pair this phone first.");
        JSONArray files = client.files();
        String catalog = files.toString();
        if (catalog.equals(lastCatalog)) return;
        lastCatalog = catalog;
        runOnUiThread(() -> {
            fileList.removeAllViews();
            if (files.length() == 0) fileList.addView(text("NO FILES SELECTED", 12, false));
            for (int i = 0; i < files.length(); i++) {
                JSONObject file = files.optJSONObject(i);
                if (file == null) continue;
                String id = file.optString("id");
                String name = file.optString("name", "file");
                long size = file.optLong("size");
                LinearLayout card = new LinearLayout(this);
                card.setOrientation(LinearLayout.VERTICAL);
                card.setPadding(dp(12), dp(12), dp(12), dp(12));
                card.setBackground(border());
                LinearLayout.LayoutParams cardParams = new LinearLayout.LayoutParams(-1, -2);
                cardParams.topMargin = dp(10);
                fileList.addView(card, cardParams);
                card.addView(text(name, 15, true));
                card.addView(text(String.format(Locale.US, "%s  •  %.1f MiB", id.substring(0, Math.min(8, id.length())), size / 1048576.0), 11, false));
                card.addView(button("[ SAVE TO DOWNLOADS ]", false, view -> download(id, name, size)));
            }
        });
    }

    private void download(String id, String name, long size) {
        work("DOWNLOADING " + name, () -> {
            showCancel(true);
            Uri uri = null;
            try {
                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, name.replace('/', '_').replace('\\', '_'));
                values.put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/TapSend");
                values.put(MediaStore.MediaColumns.IS_PENDING, 1);
                uri = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (uri == null) throw new IllegalStateException("Could not create a Downloads file.");
                try (OutputStream output = getContentResolver().openOutputStream(uri)) {
                    if (output == null) throw new IllegalStateException("Could not write to Downloads.");
                    client.download(id, size, output, (done, total) -> progress("RECEIVING " + name, done, total));
                }
                values.clear();
                values.put(MediaStore.MediaColumns.IS_PENDING, 0);
                if (getContentResolver().update(uri, values, null, null) != 1) {
                    throw new IllegalStateException("Could not finish saving the Downloads file.");
                }
                client.receipt(id);
                postStatus("Saved to Downloads/TapSend: " + name);
            } catch (Exception error) {
                if (uri != null) getContentResolver().delete(uri, null, null);
                throw error;
            } finally { showCancel(false); }
        });
    }

    private void chooseFiles() {
        if (!client.hasIdentity()) { postStatus("Pair this phone first."); selectTab(TAB_CONNECTION); return; }
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.setType("*/*");
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        startActivityForResult(intent, PICK_FILE);
    }

    private void startPeerShare() {
        if (peerServer != null) { postStatus("Phone share is already running."); return; }
        nearbyWork("STARTING PHONE SHARE", () -> {
            PeerShareServer created = new PeerShareServer(getContentResolver());
            peerServer = created;
            runOnUiThread(() -> {
                peerShareLink.setText(created.url());
                showPeerQr(created.url());
                peerShareCount.setText(created.count() + " FILES SHARED");
                updatePeerJoinRequests();
            });
            postStatus("Share ready. Add files and copy the link.");
        });
    }

    private void stopPeerShare() {
        PeerShareServer old = peerServer;
        peerServer = null;
        if (old != null) old.close();
        peerShareLink.setText("SHARING OFF");
        peerShareQr.setImageDrawable(null);
        peerShareQr.setVisibility(View.GONE);
        peerShareCount.setText("0 FILES SHARED");
        peerJoinSummary.setText("0 RECEIVERS");
        peerJoinRequests.removeAllViews();
        lastPeerJoins = "";
        postStatus("Phone share stopped. The old link no longer works.");
    }

    private void updatePeerJoinRequests() {
        PeerShareServer current = peerServer;
        if (current == null) return;
        List<String> pending = current.pending();
        String signature = current.joinedCount() + ":" + String.join(",", pending);
        if (signature.equals(lastPeerJoins)) return;
        lastPeerJoins = signature;
        peerJoinSummary.setText(current.joinedCount() + " RECEIVERS  /  " + pending.size() + " WAITING");
        peerJoinRequests.removeAllViews();
        for (String address : pending) {
            peerJoinRequests.addView(text("JOIN REQUEST  " + address, 12, true));
            LinearLayout actions = actionRow(peerJoinRequests);
            rowButton(actions, "[ APPROVE ]", true, view -> {
                current.approve(address);
                updatePeerJoinRequests();
                postStatus("Approved " + address);
            });
            rowButton(actions, "[ DENY ]", false, view -> {
                current.deny(address);
                updatePeerJoinRequests();
                postStatus("Denied " + address);
            });
        }
    }

    private void choosePeerFiles() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.setType("*/*");
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        startActivityForResult(intent, PICK_PEER_FILE);
    }

    private void copyPeerLink() {
        PeerShareServer current = peerServer;
        if (current == null) { postStatus("Start a phone share first."); return; }
        android.content.ClipboardManager clipboard = (android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        clipboard.setPrimaryClip(ClipData.newPlainText("TapSend phone link", current.url()));
        postStatus("Link copied. Open it on the other phone's browser.");
    }

    private void showPeerQr(String link) {
        try {
            BitMatrix code = new QRCodeWriter().encode(link, BarcodeFormat.QR_CODE, 440, 440);
            Bitmap bitmap = Bitmap.createBitmap(440, 440, Bitmap.Config.ARGB_8888);
            for (int y = 0; y < 440; y++) {
                for (int x = 0; x < 440; x++) bitmap.setPixel(x, y, code.get(x, y) ? Color.BLACK : Color.WHITE);
            }
            peerShareQr.setImageBitmap(bitmap);
            peerShareQr.setVisibility(View.VISIBLE);
        } catch (Exception error) { postStatus("QR unavailable. Copy the link instead."); }
    }

    private void scanPeerLink() {
        GmsBarcodeScanning.getClient(this).startScan()
                .addOnSuccessListener(code -> {
                    String value = code.getRawValue();
                    if (value != null) { peerLinkInput.setText(value); openPeerLink(); }
                })
                .addOnFailureListener(error -> postStatus("Scanner unavailable: " + detail(error)));
    }

    private void openPeerLink() {
        final PeerShareClient incoming;
        try { incoming = new PeerShareClient(peerLinkInput.getText().toString()); }
        catch (Exception error) { postStatus(detail(error)); return; }
        peerClient = incoming;
        lastPeerCatalog = "";
        nearbyWork("CONNECTING TO PHONE", () -> refreshPeerFiles(incoming));
    }

    private void refreshPeerFiles(PeerShareClient incoming) throws Exception {
        JSONArray files = incoming.files();
        if (incoming != peerClient) return;
        String catalog = files.toString();
        if (catalog.equals(lastPeerCatalog)) return;
        lastPeerCatalog = catalog;
        getPreferences(0).edit().putString("peer_link", incoming.base()).apply();
        runOnUiThread(() -> {
            peerFileList.removeAllViews();
            if (files.length() == 0) peerFileList.addView(text("NO FILES SHARED YET", 12, false));
            for (int i = 0; i < files.length(); i++) {
                JSONObject item = files.optJSONObject(i);
                if (item == null) continue;
                int id = item.optInt("id", -1);
                String name = item.optString("name", "file");
                long size = item.optLong("size", -1);
                Button save = button("[ SAVE " + name + " ]", false, view -> downloadPeerFile(incoming, id, name, size));
                save.setSingleLine(false);
                peerFileList.addView(save);
            }
        });
        postStatus("Connected to phone. Tap a file to save it.");
    }

    private void downloadPeerFile(PeerShareClient incoming, int id, String name, long size) {
        nearbyWork("RECEIVING " + name, () -> {
            Uri destination = null;
            try {
                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, name.replace('/', '_').replace('\\', '_'));
                values.put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/TapSend");
                values.put(MediaStore.MediaColumns.IS_PENDING, 1);
                destination = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (destination == null) throw new IllegalStateException("Could not create a Downloads file.");
                try (OutputStream output = getContentResolver().openOutputStream(destination)) {
                    if (output == null) throw new IllegalStateException("Could not write to Downloads.");
                    incoming.download(id, output, done -> progress("RECEIVING " + name, done, size));
                }
                values.clear();
                values.put(MediaStore.MediaColumns.IS_PENDING, 0);
                if (getContentResolver().update(destination, values, null, null) != 1)
                    throw new IllegalStateException("Could not finish saving the file.");
                postStatus("Saved to Downloads/TapSend: " + name);
            } catch (Exception error) {
                if (destination != null) getContentResolver().delete(destination, null, null);
                throw error;
            }
        });
    }

    @Override protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if ((requestCode != PICK_FILE && requestCode != PICK_PEER_FILE) || resultCode != RESULT_OK || data == null) return;
        List<Uri> selected = new ArrayList<>();
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int i = 0; i < clip.getItemCount(); i++) selected.add(clip.getItemAt(i).getUri());
        } else if (data.getData() != null) selected.add(data.getData());
        if (requestCode == PICK_PEER_FILE) {
            nearbyWork("ADDING PHONE FILES", () -> {
                PeerShareServer current = peerServer;
                if (current == null) {
                    current = new PeerShareServer(getContentResolver());
                    peerServer = current;
                }
                for (Uri uri : selected) {
                    String name = "file";
                    long size = -1;
                    try (Cursor cursor = getContentResolver().query(uri,
                            new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE}, null, null, null)) {
                        if (cursor != null && cursor.moveToFirst()) {
                            name = cursor.getString(0);
                            if (!cursor.isNull(1)) size = cursor.getLong(1);
                        }
                    }
                    current.add(new PeerShareServer.File(uri, name, size));
                }
                PeerShareServer active = current;
                runOnUiThread(() -> {
                    peerShareLink.setText(active.url());
                    showPeerQr(active.url());
                    peerShareCount.setText(active.count() + " FILES SHARED");
                });
                postStatus("Phone files ready. Copy the link to the other phone.");
            });
            return;
        }
        work("UPLOADING", () -> {
            showCancel(true);
            try {
                for (Uri uri : selected) {
                    String name = "file";
                    long size = -1;
                    try (Cursor cursor = getContentResolver().query(uri,
                            new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE}, null, null, null)) {
                        if (cursor != null && cursor.moveToFirst()) {
                            name = cursor.getString(0);
                            if (!cursor.isNull(1)) size = cursor.getLong(1);
                        }
                    }
                    String uploadName = name;
                    client.upload(name, size, () -> getContentResolver().openInputStream(uri),
                            (done, total) -> progress("SENDING " + uploadName, done, total));
                    postStatus("Saved on computer: " + name);
                    String completed = name;
                    runOnUiThread(() -> {
                        if (uploadHistory.getChildCount() == 1 &&
                                ((TextView) uploadHistory.getChildAt(0)).getText().toString().equals("NO UPLOADS THIS SESSION")) {
                            uploadHistory.removeAllViews();
                        }
                        uploadHistory.addView(text("↑  " + completed, 12, false));
                    });
                }
            } finally { showCancel(false); }
        });
    }

    private void sendMessage() {
        if (!client.hasIdentity()) { postStatus("Pair this phone first."); selectTab(TAB_CONNECTION); return; }
        String text = messageInput.getText().toString().trim();
        if (text.isEmpty()) return;
        work("SENDING MESSAGE", () -> {
            client.message(text);
            runOnUiThread(() -> messageInput.setText(""));
            refreshMessages();
            postStatus("Message sent.");
        });
    }

    private void poll() {
        if (destroyed) return;
        worker.execute(() -> {
            if (client.hasIdentity()) {
                try {
                    boolean stopped = client.stopped();
                    runOnUiThread(() -> session.setText(stopped ? "TRANSFER STOPPED ON COMPUTER" : "READY TO RECEIVE"));
                    refreshMessages();
                    refreshFiles();
                } catch (Exception error) {
                    runOnUiThread(() -> session.setText("CONNECTION LOST: " + detail(error)));
                }
            }
            if (!destroyed) handler.postDelayed(this::poll, 3000);
        });
    }

    private void pollNearby() {
        if (destroyed) return;
        updatePeerJoinRequests();
        PeerShareClient incoming = peerClient;
        if (incoming != null && peerPollBusy.compareAndSet(false, true)) {
            peerWorker.execute(() -> {
                try { refreshPeerFiles(incoming); }
                catch (Exception error) { postStatus(detail(error)); }
                finally { peerPollBusy.set(false); }
            });
        }
        handler.postDelayed(this::pollNearby, 3000);
    }

    private void refreshMessages() throws Exception {
        JSONArray messages = client.messages();
        long last = messages.length() == 0 ? 0 : messages.getJSONObject(messages.length() - 1).getLong("id");
        if (last == latestMessageId) return;
        latestMessageId = last;
        runOnUiThread(() -> {
            thread.removeAllViews();
            if (messages.length() == 0) thread.addView(text("NO MESSAGES YET", 12, false));
            for (int i = 0; i < messages.length(); i++) {
                JSONObject item = messages.optJSONObject(i);
                if (item == null) continue;
                String from = item.optString("from").equals("phone") ? "PHONE" : "COMPUTER";
                TextView line = text(from + " > " + item.optString("text"), 12, false);
                line.setPadding(dp(8), dp(8), dp(8), dp(8));
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
                params.topMargin = dp(6);
                line.setBackground(border());
                thread.addView(line, params);
            }
        });
    }

    private void work(String label, Job job) {
        postStatus(label);
        worker.execute(() -> {
            try { job.run(); }
            catch (Exception error) { postStatus(detail(error)); }
        });
    }

    private void nearbyWork(String label, Job job) {
        postStatus(label);
        peerWorker.execute(() -> {
            try { job.run(); }
            catch (Exception error) { postStatus(detail(error)); }
        });
    }

    private void progress(String label, long done, long total) {
        String value = total > 0 ? String.format(Locale.US, "%s  %d%%", label, done * 100 / total)
                : String.format(Locale.US, "%s  %.1f MiB", label, done / 1048576.0);
        postStatus(value);
    }

    private void showCancel(boolean visible) {
        runOnUiThread(() -> cancelButton.setVisibility(visible ? View.VISIBLE : View.GONE));
    }

    private void postStatus(String value) {
        runOnUiThread(() -> status.setText(value));
    }

    private static String detail(Exception error) {
        String message = error.getMessage();
        return message == null || message.isBlank() ? error.getClass().getSimpleName() : message;
    }

    private TextView text(String value, int size, boolean bold) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextColor(Color.WHITE);
        view.setTextSize(size);
        view.setTypeface(Typeface.MONOSPACE, bold ? Typeface.BOLD : Typeface.NORMAL);
        view.setPadding(0, dp(5), 0, dp(5));
        return view;
    }

    private EditText input(String hint) {
        EditText view = new EditText(this);
        view.setSingleLine(false);
        view.setMinLines(1);
        view.setMaxLines(3);
        view.setHint(hint);
        view.setHintTextColor(Color.WHITE);
        view.setTextColor(Color.WHITE);
        view.setTextSize(13);
        view.setTypeface(Typeface.MONOSPACE);
        view.setPadding(dp(12), dp(10), dp(12), dp(10));
        view.setBackground(border());
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(12);
        view.setLayoutParams(params);
        return view;
    }

    private Button button(String label, boolean filled, View.OnClickListener click) {
        Button view = new Button(this);
        view.setAllCaps(false);
        view.setText(label);
        view.setTextSize(12);
        view.setTypeface(Typeface.MONOSPACE, Typeface.BOLD);
        view.setTextColor(filled ? Color.BLACK : Color.WHITE);
        GradientDrawable background = border();
        if (filled) background.setColor(Color.WHITE);
        view.setBackground(background);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, dp(48));
        params.topMargin = dp(10);
        view.setLayoutParams(params);
        view.setOnClickListener(click);
        return view;
    }

    private GradientDrawable border() {
        return border(dp(12));
    }

    private GradientDrawable border(int radius) {
        GradientDrawable shape = new GradientDrawable();
        shape.setColor(Color.BLACK);
        shape.setStroke(dp(1), Color.WHITE);
        shape.setCornerRadius(radius);
        return shape;
    }

    private void heading(LinearLayout parent, String title) {
        parent.addView(text(title, 16, true));
    }

    private void divider(LinearLayout parent) {
        View line = new View(this);
        line.setBackgroundColor(Color.WHITE);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, dp(1));
        params.topMargin = dp(22);
        params.bottomMargin = dp(20);
        parent.addView(line, params);
    }

    private int dp(int value) {
        return Math.round(getResources().getDisplayMetrics().density * value);
    }

    private interface Job { void run() throws Exception; }
}
