package com.tapsend.android;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Color;
import android.graphics.Canvas;
import android.graphics.Paint;
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

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
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
    private static final int PICK_PORTAL_FILE = 9;
    private static final int SCREEN_CONSENT = 10;
    private static final int SCREEN_TOOL = 90;
    private static final int SCREEN_PEN_TOOL = 91;
    private static final int SCREEN_PEN_CONSENT = 11;
    private static final int TAB_DASHBOARD = 4;
    private static final int TAB_SEND = 0;
    private static final int TAB_RECEIVE = 1;
    private static final int TAB_CONNECTION = 2;
    private static final int TAB_MESSAGES = 3;
    private static final int TAB_SHARE = 5;

    private static final int BACKGROUND = 0xff0d0d0d;
    private static final int SURFACE = 0xff141414;
    private static final int INK = 0xffeeeeee;
    private static final int MUTED = 0xffa0a0a0;
    private static final int BORDER = 0xffbdbdbd;
    private static final int ACCENT = Color.WHITE;
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
    private TextView pairingNext;
    private EditText pairingDeviceName;
    private TextView session;
    private LinearLayout fileList;
    private LinearLayout thread;
    private LinearLayout scratchpadPanel, scratchpadRows;
    private String scratchpadSnapshot = "";
    private LinearLayout uploadHistory;
    private LinearLayout selectedFiles;
    private final List<Uri> sendQueue = new ArrayList<>();
    private final List<Uri> incomingShare = new ArrayList<>();
    private final ArrayList<String> incomingPasteText = new ArrayList<>();
    private final List<Uri> nearbyQueue = new ArrayList<>();
    private LinearLayout sharedFiles, nearbyQueuedFiles;
    private Button shareToComputer, shareToNearby, confirmNearby;
    private Button shareToPasteStack;
    private boolean stagingPaste;
    private TextView portalTile;
    private final List<Uri> portalPending = new ArrayList<>();
    private boolean portalSending;
    private boolean queueSending;
    private LinearLayout peerFileList;
    private EditText peerLinkInput;
    private TextView peerShareLink;
    private TextView peerShareCount;
    private LinearLayout peerShareDetails;
    private Button startShareButton;
    private TextView peerJoinSummary;
    private TextView peerTransferSummary;
    private LinearLayout peerJoinRequests;
    private String lastPeerJoins = "";
    private ImageView peerShareQr;
    private volatile PeerShareServer peerServer;
    private volatile PeerShareClient peerClient;
    private volatile String lastPeerCatalog = "";
    private final LinearLayout[] pages = new LinearLayout[6];
    private LinearLayout pageContent, consoleWork, sendSourceControls, receiveSourceControls;
    private ScrollView contentScroll;
    private int activeTab = -1;
    private int consolePage = -1;
    private boolean consoleNearby;
    private boolean sendNearby, receiveNearby, returnToSendAfterPairing, preparingNearby;
    private Button cancelButton;
    private Button sendQueueButton, computerTab, nearbyTab;
    private Button reconnectButton;
    private LinearLayout managePairing;
    private TextView dashboardLogs, consoleStatus, consoleDestination, latestTransfer, receiptMark;
    private TextView sendCommandTitle, sendCommandDetail, receiveCommandTitle, receiveCommandDetail;
    private View sendCommand, receiveCommand;
    private DeviceGlyph destinationIcon;
    private int computerFileCount;
    private android.widget.ProgressBar transferProgress;
    private LinearLayout statusPanel;
    private java.util.function.Consumer<Boolean> selectSendSource, selectReceiveSource;
    private final java.util.ArrayDeque<String> eventLog = new java.util.ArrayDeque<>();
    private long sampleTime, sampleBytes, lastProgressUi;
    private String sampleLabel = "";
    private boolean computerOnline;
    private volatile long latestMessageId = -1;
    private volatile String lastCatalog = "";
    private volatile boolean targetedShare;
    private volatile boolean destroyed;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        client = new TapSendClient(new IdentityStore(this));
        buildUi();
        if (PasteOutbox.root(this).isDirectory()) PasteDelivery.schedule(this, false);
        work("Checking for a paired computer…", () -> {
            client.loadAndAuthenticate();
            if (client.hasIdentity()) {
                showConnected();
                refreshFiles();
            } else {
                runOnUiThread(() -> {
                    if (activeTab == TAB_DASHBOARD) statusPanel.setVisibility(View.GONE);
                });
            }
        });
        handler.postDelayed(this::poll, 2500);
        handler.postDelayed(this::pollNearby, 1500);
        if (state == null) openShareIntent(getIntent());
        else {
            restoreQueue(state, "computer", sendQueue);
            restoreQueue(state, "incoming", incomingShare);
            ArrayList<String> savedText = state.getStringArrayList("pasteText");
            if (savedText != null) incomingPasteText.addAll(savedText);
            restoreQueue(state, "nearby", nearbyQueue);
            returnToSendAfterPairing = state.getBoolean("returnToSend");
            consoleNearby = state.getBoolean("consoleNearby");
            updateConsole();
            renderSendQueue();
            renderIncomingShare();
            renderNearbyQueue();
            showDestination(state.getInt("page", TAB_DASHBOARD), state.getBoolean("nearbyDestination"));
        }
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        openShareIntent(intent);
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putParcelableArrayList("computer", new ArrayList<>(sendQueue));
        state.putParcelableArrayList("incoming", new ArrayList<>(incomingShare));
        state.putStringArrayList("pasteText", new ArrayList<>(incomingPasteText));
        state.putParcelableArrayList("nearby", new ArrayList<>(nearbyQueue));
        state.putInt("page", activeTab);
        state.putBoolean("nearbyDestination", activeTab == TAB_SEND ? sendNearby : receiveNearby);
        state.putBoolean("returnToSend", returnToSendAfterPairing);
        state.putBoolean("consoleNearby", consoleNearby);
    }

    private static void restoreQueue(Bundle state, String key, List<Uri> queue) {
        List<Uri> saved = state.getParcelableArrayList(key, Uri.class);
        if (saved != null) queue.addAll(saved);
    }

    @Override protected void onDestroy() {
        destroyed = true;
        handler.removeCallbacksAndMessages(null);
        client.cancelTransfer();
        if (peerServer != null) peerServer.close();
        worker.shutdownNow();
        peerWorker.shutdownNow();
        super.onDestroy();
    }

    private void buildUi() {
        LinearLayout screen = column();
        screen.setBackgroundColor(BACKGROUND);
        screen.setFocusableInTouchMode(true);
        screen.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets system = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            int bottom = Math.max(system.bottom, insets.getInsets(WindowInsets.Type.ime()).bottom);
            view.setPadding(system.left, system.top, system.right, bottom);
            return insets;
        });
        setContentView(screen);

        LinearLayout header = new LinearLayout(this);
        header.setGravity(android.view.Gravity.CENTER_VERTICAL);
        header.setPadding(dp(16), dp(8), dp(16), dp(8));
        screen.addView(header, new LinearLayout.LayoutParams(-1, -2));
        LinearLayout brand = new LinearLayout(this);
        brand.setGravity(android.view.Gravity.CENTER_VERTICAL);
        brand.setMinimumHeight(dp(44));
        brand.setFocusable(true);
        brand.setContentDescription("TapSend home");
        brand.setOnClickListener(view -> selectTab(TAB_DASHBOARD));
        TextView prompt = text(">_", 14, true);
        prompt.setGravity(android.view.Gravity.CENTER);
        prompt.setPadding(0, 0, 0, 0);
        prompt.setTextColor(BACKGROUND);
        prompt.setBackgroundColor(INK);
        brand.addView(prompt, new LinearLayout.LayoutParams(dp(30), dp(24)));
        TextView brandName = text("TapSend", 15, true);
        LinearLayout.LayoutParams brandNameParams = new LinearLayout.LayoutParams(-2, -2);
        brandNameParams.leftMargin = dp(10);
        brand.addView(brandName, brandNameParams);
        header.addView(brand, new LinearLayout.LayoutParams(0, -2, 1));
        Button tools = button("Tools", false, this::openTools);
        tools.setText("⌘ Tools");
        header.addView(tools, new LinearLayout.LayoutParams(-2, -2));
        addRule(screen);

        contentScroll = new ScrollView(this);
        contentScroll.setFillViewport(true);
        contentScroll.setClipToPadding(false);
        contentScroll.setPadding(dp(16), 0, dp(16), 0);
        screen.addView(contentScroll, new LinearLayout.LayoutParams(-1, 0, 1));
        pageContent = column();
        contentScroll.addView(pageContent);
        pages[TAB_DASHBOARD] = buildDashboardPage();
        pages[TAB_SEND] = buildSendPage();
        pages[TAB_RECEIVE] = buildReceivePage();
        pages[TAB_CONNECTION] = buildConnectionPage();
        pages[TAB_MESSAGES] = buildMessagesPage();
        pages[TAB_SHARE] = buildSharePage();
        for (LinearLayout page : pages) pageContent.addView(page);

        statusPanel = panel(screen, "Status");
        status = text("Connecting…", 13, true);
        status.setMaxLines(3);
        status.setEllipsize(android.text.TextUtils.TruncateAt.END);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        statusPanel.addView(status);
        transferProgress = new android.widget.ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        transferProgress.setMax(100);
        transferProgress.setVisibility(View.GONE);
        statusPanel.addView(transferProgress, new LinearLayout.LayoutParams(-1, dp(5)));
        cancelButton = button("Cancel transfer", false, view -> client.cancelTransfer());
        cancelButton.setVisibility(View.GONE);
        statusPanel.addView(cancelButton);
        getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> {
                    if (activeTab == TAB_DASHBOARD && consolePage != -1) closeConsoleCommand();
                    else if (activeTab == TAB_DASHBOARD) finish();
                    else selectTab(TAB_DASHBOARD);
                });
        selectTab(TAB_DASHBOARD);
        updateCommandOutline();
        updateDashboard();
        screen.requestFocus();
    }

    private void openTools(View anchor) {
        android.widget.PopupMenu menu = new android.widget.PopupMenu(this, anchor);
        menu.getMenu().add(0, TAB_CONNECTION, 0, "Connect a PC");
        menu.getMenu().add(0, TAB_MESSAGES, 1, "Messages");
        menu.getMenu().add(0, SCREEN_TOOL, 2,
                ScreenCaptureService.isActive() ? "Stop screen sharing" : "Share screen to PC");
        if (!ScreenCaptureService.isActive()) menu.getMenu().add(0, SCREEN_PEN_TOOL, 3, "Share screen with Live Red Pen");
        if (!incomingShare.isEmpty() || !incomingPasteText.isEmpty()) menu.getMenu().add(0, TAB_SHARE, 2, "Items from Android Share");
        menu.setOnMenuItemClickListener(item -> {
            if (item.getItemId() == SCREEN_TOOL || item.getItemId() == SCREEN_PEN_TOOL)
                toggleScreenSharing(item.getItemId() == SCREEN_PEN_TOOL);
            else selectTab(item.getItemId());
            return true;
        });
        menu.show();
    }

    private void toggleScreenSharing(boolean allowPen) {
        Intent service = new Intent(this, ScreenCaptureService.class);
        if (ScreenCaptureService.isActive()) {
            stopService(service);
            postStatus("Screen sharing stopped.");
        } else if (!client.hasIdentity()) {
            postStatus("Pair a computer before sharing your screen.");
            selectTab(TAB_CONNECTION);
        } else {
            if (allowPen && !android.provider.Settings.canDrawOverlays(this)) {
                postStatus("Allow TapSend to display over other apps, then choose Live Red Pen again.");
                startActivity(new Intent(android.provider.Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:" + getPackageName())));
                return;
            }
            postStatus("On Windows, select this phone with V and open Screen Share with L.");
            android.media.projection.MediaProjectionManager manager =
                    getSystemService(android.media.projection.MediaProjectionManager.class);
            Intent capture = allowPen && android.os.Build.VERSION.SDK_INT >= 34
                    ? manager.createScreenCaptureIntent(android.media.projection.MediaProjectionConfig.createConfigForDefaultDisplay())
                    : manager.createScreenCaptureIntent();
            startActivityForResult(capture, allowPen ? SCREEN_PEN_CONSENT : SCREEN_CONSENT);
        }
    }

    private LinearLayout buildSharePage() {
        LinearLayout page = page("Shared with TapSend");
        LinearLayout review = panel(page, "Review shared items");
        sharedFiles = column();
        review.addView(sharedFiles);
        shareToComputer = button("Computer  →", true, view -> {
            addToQueue(sendQueue, incomingShare);
            incomingShare.clear();
            renderIncomingShare();
            renderSendQueue();
            showDestination(TAB_SEND, false);
        });
        shareToNearby = button("Nearby phone  →", false, view -> {
            addToQueue(nearbyQueue, incomingShare);
            incomingShare.clear();
            renderIncomingShare();
            renderNearbyQueue();
            showDestination(TAB_SEND, true);
        });
        review.addView(shareToComputer);
        review.addView(shareToNearby);
        shareToPasteStack = button("Add to Paste Stack", false, view -> queuePasteStack());
        review.addView(shareToPasteStack);
        renderIncomingShare();
        return page;
    }

    private void openShareIntent(Intent intent) {
        String action = intent == null ? null : intent.getAction();
        if (!Intent.ACTION_SEND.equals(action) && !Intent.ACTION_SEND_MULTIPLE.equals(action)) return;
        java.util.LinkedHashSet<Uri> files = new java.util.LinkedHashSet<>();
        java.util.LinkedHashSet<String> texts = new java.util.LinkedHashSet<>();
        try {
            if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
                ArrayList<Uri> streams = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri.class);
                if (streams != null) files.addAll(streams);
            } else {
                files.add(intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri.class));
            }
            ClipData clip = intent.getClipData();
            CharSequence sharedText = intent.getCharSequenceExtra(Intent.EXTRA_TEXT);
            if (sharedText != null && sharedText.length() <= 16384 && !sharedText.toString().isBlank()) {
                texts.add(sharedText.toString());
            }
            if (clip != null) {
                for (int index = 0; index < clip.getItemCount(); index++) {
                    ClipData.Item item = clip.getItemAt(index);
                    files.add(item.getUri());
                    CharSequence itemText = item.getText();
                    if (itemText != null && itemText.length() <= 16384 && !itemText.toString().isBlank()) {
                        texts.add(itemText.toString());
                    }
                }
            }
        } catch (RuntimeException error) {
            postStatus("TapSend couldn't read this share. Try sharing the files again.");
            return;
        }
        files.removeIf(uri -> uri == null || !"content".equals(uri.getScheme()));
        if (files.isEmpty() && texts.isEmpty()) {
            postStatus("Share a screenshot, text, or file with TapSend.");
            return;
        }
        addToQueue(incomingShare, files);
        for (String value : texts) if (!incomingPasteText.contains(value)) incomingPasteText.add(value);
        renderIncomingShare();
        selectTab(TAB_SHARE);
    }

    private static void addToQueue(List<Uri> queue, Iterable<Uri> files) {
        for (Uri file : files) if (!queue.contains(file)) queue.add(file);
    }

    private void renderIncomingShare() {
        renderFileSelection(sharedFiles, incomingShare, stagingPaste, this::renderIncomingShare);
        boolean any = !incomingShare.isEmpty();
        shareToComputer.setEnabled(any && !stagingPaste);
        shareToNearby.setEnabled(any && !stagingPaste);
        shareToPasteStack.setEnabled(!stagingPaste && (any || !incomingPasteText.isEmpty()));
        for (String value : incomingPasteText) {
            TextView preview = text(value, 13, false);
            preview.setMaxLines(3);
            sharedFiles.addView(preview);
            Button remove = button("Remove text", false, view -> {
                incomingPasteText.remove(value);
                renderIncomingShare();
            });
            remove.setEnabled(!stagingPaste);
            sharedFiles.addView(remove);
        }
        if (!any && incomingPasteText.isEmpty()) sharedFiles.addView(text("No items selected.", 13, false));
    }

    private void queuePasteStack() {
        if (stagingPaste) return;
        List<Uri> files = List.copyOf(incomingShare);
        List<String> texts = List.copyOf(incomingPasteText);
        stagingPaste = true;
        renderIncomingShare();
        work("Saving to Paste Stack…", () -> {
            try {
                int count = PasteOutbox.enqueue(getApplicationContext(), files, texts);
                boolean scheduled = PasteDelivery.schedule(getApplicationContext(), true);
                runOnUiThread(() -> {
                    incomingShare.removeAll(files);
                    incomingPasteText.removeAll(texts);
                    renderIncomingShare();
                });
                postStatus(scheduled
                        ? "Queued " + count + " for Paste Stack. They’ll send when your PC is reachable."
                        : "Saved for Paste Stack. Reopen TapSend to retry delivery.");
            } finally {
                runOnUiThread(() -> {
                    stagingPaste = false;
                    renderIncomingShare();
                });
            }
        });
    }

    private void renderNearbyQueue() {
        renderFileSelection(nearbyQueuedFiles, nearbyQueue, preparingNearby, this::renderNearbyQueue);
        confirmNearby.setVisibility(nearbyQueue.isEmpty() ? View.GONE : View.VISIBLE);
        confirmNearby.setEnabled(!preparingNearby);
        commandLabel(confirmNearby, peerServer == null ? "Create QR and link" : "Add files to active share");
    }

    private LinearLayout buildSendPage() {
        LinearLayout page = page("Send files");
        LinearLayout computer = column();
        LinearLayout nearby = column();
        sendSourceControls = column();
        page.addView(sendSourceControls);
        selectSendSource = addSources(page, sendSourceControls, computer, nearby, "Send to", value -> sendNearby = value);
        LinearLayout selection = panel(computer, "Files to send");
        selectedFiles = column();
        selection.addView(button("Choose files", true, view -> chooseFiles()));
        selection.addView(selectedFiles);
        sendQueueButton = button("Send files", true, view -> {
            if (queueSending || sendQueue.isEmpty()) return;
            if (!client.hasIdentity()) {
                returnToSendAfterPairing = true;
                postStatus("Pair a computer to send these files.");
                selectTab(TAB_CONNECTION);
                return;
            }
            queueSending = true;
            renderSendQueue();
            uploadSelected(new ArrayList<>(sendQueue));
        });
        selection.addView(sendQueueButton);
        renderSendQueue();

        uploadHistory = column();
        uploadHistory.addView(text("No files sent to the computer yet.", 14, false));
        disclosure(computer, "Sent files", uploadHistory);

        LinearLayout portal = column();
        portal.addView(text("Use this when Portal Mode is open on your computer.", 14, false));
        portal.addView(button("Choose portal files", false, view -> choosePortalFiles()));
        portalTile = text("Tap or swipe up to send", 15, true);
        portalTile.setGravity(android.view.Gravity.CENTER);
        portalTile.setPadding(dp(12), dp(16), dp(12), dp(16));
        portalTile.setBackground(new android.graphics.drawable.RippleDrawable(
                android.content.res.ColorStateList.valueOf(0x28ffffff), shape(SURFACE, ACCENT), null));
        portalTile.setVisibility(View.GONE);
        LinearLayout.LayoutParams tileParams = new LinearLayout.LayoutParams(-1, -2);
        tileParams.topMargin = dp(10);
        portal.addView(portalTile, tileParams);
        portalTile.setOnTouchListener(new View.OnTouchListener() {
            float startY;
            @Override public boolean onTouch(View view, android.view.MotionEvent event) {
                if (event.getActionMasked() == android.view.MotionEvent.ACTION_DOWN) {
                    startY = event.getY();
                    view.getParent().requestDisallowInterceptTouchEvent(true);
                    return true;
                }
                if (event.getActionMasked() == android.view.MotionEvent.ACTION_UP) {
                    view.getParent().requestDisallowInterceptTouchEvent(false);
                    if (startY - event.getY() > dp(45)) sendPortalFiles();
                    else view.performClick();
                    return true;
                }
                if (event.getActionMasked() == android.view.MotionEvent.ACTION_CANCEL) {
                    view.getParent().requestDisallowInterceptTouchEvent(false);
                }
                return true;
            }
        });
        portalTile.setOnClickListener(view -> sendPortalFiles());
        disclosure(computer, "Portal Mode", portal);
        buildNearbySendPanel(nearby);
        return page;
    }

    private void buildNearbySendPanel(LinearLayout page) {
        LinearLayout nearby = panel(page, "Share over Wi-Fi");
        nearbyQueuedFiles = column();
        nearby.addView(nearbyQueuedFiles);
        confirmNearby = button("Create QR and link", true, view -> {
            if (preparingNearby || nearbyQueue.isEmpty()) return;
            List<Uri> selected = List.copyOf(nearbyQueue);
            shareNearby(selected, true);
        });
        confirmNearby.setVisibility(View.GONE);
        nearby.addView(confirmNearby);
        nearby.addView(text("Choose files, then show the QR or share the link with the receiving device.", 14, false));
        nearby.addView(button("Choose files", true, view -> choosePeerFiles()));
        startShareButton = button("Start share before adding files", false, view -> startPeerShare());
        nearby.addView(startShareButton);
        peerShareDetails = column();
        peerShareDetails.setVisibility(View.GONE);
        nearby.addView(peerShareDetails);
        peerShareQr = new ImageView(this);
        peerShareQr.setContentDescription("Phone share QR code");
        peerShareQr.setVisibility(View.GONE);
        LinearLayout.LayoutParams qrParams = new LinearLayout.LayoutParams(dp(196), dp(196));
        qrParams.gravity = android.view.Gravity.CENTER_HORIZONTAL;
        qrParams.topMargin = dp(12);
        peerShareDetails.addView(peerShareQr, qrParams);
        peerShareCount = text("No files shared yet", 14, true);
        peerShareDetails.addView(peerShareCount);
        LinearLayout actions = actionRow(peerShareDetails);
        rowButton(actions, "Copy link", true, view -> copyPeerLink());
        rowButton(actions, "Stop share", false, view -> stopPeerShare());
        peerShareLink = text("", 13, false);
        peerShareLink.setTextIsSelectable(true);
        LinearLayout link = column();
        link.addView(peerShareLink);
        disclosure(peerShareDetails, "Share link", link);
        peerJoinSummary = text("No receivers yet", 14, false);
        peerShareDetails.addView(peerJoinSummary);
        peerJoinRequests = column();
        peerShareDetails.addView(peerJoinRequests);
        peerTransferSummary = text("No files sent yet", 14, true);
        peerShareDetails.addView(peerTransferSummary);
    }

    private LinearLayout buildReceivePage() {
        LinearLayout page = page("Receive files");
        LinearLayout computer = column();
        LinearLayout nearby = column();
        receiveSourceControls = column();
        page.addView(receiveSourceControls);
        selectReceiveSource = addSources(page, receiveSourceControls, computer, nearby, "Receive from", value -> receiveNearby = value);
        LinearLayout main = panel(computer, "Shared by your computer");
        session = text("Connect a computer to see its shared files.", 14, false);
        main.addView(session);
        fileList = column();
        main.addView(fileList);
        LinearLayout actions = actionRow(main);
        rowButton(actions, "Refresh", false, view -> {
            if (client.hasIdentity()) work("Refreshing files…", this::refreshFiles);
            else selectTab(TAB_CONNECTION);
        });
        rowButton(actions, "Connection", false, view -> selectTab(TAB_CONNECTION));
        computer.addView(text("Files are saved to Downloads/TapSend.", 14, false));
        buildNearbyReceivePanel(nearby);
        return page;
    }

    private void buildNearbyReceivePanel(LinearLayout page) {
        LinearLayout nearby = panel(page, "Open a phone share");
        nearby.addView(text("Scan the QR shown on the sending phone.", 14, false));
        nearby.addView(button("Scan share QR", true, view -> scanPeerLink()));
        peerLinkInput = input("Paste a share link");
        peerLinkInput.setText(getPreferences(0).getString("peer_link", ""));
        nearby.addView(peerLinkInput);
        nearby.addView(button("Open link", false, view -> openPeerLink()));
        peerFileList = column();
        nearby.addView(peerFileList);
        page.addView(text("Files are saved to Downloads/TapSend.", 14, false));
    }

    private LinearLayout buildConnectionPage() {
        LinearLayout page = page("Connect a computer");
        LinearLayout main = panel(page, "Connect on the same network");
        connection = text("No computer paired", 14, true);
        main.addView(connection);
        main.addView(text("Open TapSend on Windows and press P. Then scan its QR or paste the pairing pass.", 14, false));
        pairingDeviceName = input("Name for this phone");
        pairingDeviceName.setText(getPreferences(MODE_PRIVATE).getString("pairing-name", TapSendClient.defaultDeviceName()));
        main.addView(pairingDeviceName);
        main.addView(button("Scan QR", true, view -> scan()));
        main.addView(button("Paste pairing pass", false, view -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            ClipData data = clipboard.getPrimaryClip();
            CharSequence value = data == null || data.getItemCount() == 0 ? null : data.getItemAt(0).getText();
            if (value == null || value.toString().isBlank()) {
                showPairingStep("Copy the pairing pass on the PC first. Press P, then Y.");
            } else pair(value.toString());
        }));
        reconnectButton = button("Reconnect saved computer", false, view -> pairingWork("Reconnecting…", () -> {
            client.loadAndAuthenticate();
            if (!client.hasIdentity()) throw new IllegalStateException("No saved computer. Scan its pairing QR first.");
            client.session();
            showConnected();
            refreshFiles();
        }));
        main.addView(reconnectButton);
        pairingNext = text("", 14, true);
        pairingNext.setVisibility(View.GONE);
        main.addView(pairingNext);
        LinearLayout help = column();
        help.addView(text("1. Connect both devices to the same network. Use Wi-Fi on the same router if the computer is wired.", 14, false));
        help.addView(text("2. On Windows, allow TapSend through Windows Firewall on the current network profile.", 14, false));
        help.addView(text("3. If the computer changed networks or IP address, restart TapSend on the computer and scan its new pairing QR.", 14, false));
        disclosure(page, "Why won’t it connect?", help);

        LinearLayout manual = column();
        addressInput = input("Computer address");
        codeInput = input("8-digit pairing code");
        keyInput = input("Security key");
        codeInput.setInputType(android.text.InputType.TYPE_CLASS_NUMBER);
        keyInput.setInputType(android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS
                | android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD);
        manual.addView(addressInput);
        manual.addView(codeInput);
        manual.addView(keyInput);
        manual.addView(button("Pair with code", true, view -> pairManual()));
        disclosure(page, "Enter pairing code", manual);

        LinearLayout link = column();
        linkInput = input("Enter pairing pass or link");
        link.addView(linkInput);
        link.addView(button("Connect with pass", true, view -> pair(linkInput.getText().toString())));
        disclosure(page, "Enter a pass manually", link);

        LinearLayout saved = column();
        saved.addView(button("Forget paired computer", false, view -> {
            client.cancelTransfer();
            work("Removing pairing…", () -> {
            stopService(new Intent(this, ScreenCaptureService.class));
            client.forget();
            scratchpadSnapshot = "";
                runOnUiThread(() -> {
                scratchpadRows.removeAllViews();
                scratchpadPanel.setVisibility(View.GONE);
                    fileList.removeAllViews();
                    thread.removeAllViews();
                    thread.addView(text("No messages yet.", 14, false));
                    latestMessageId = -1;
                    lastCatalog = "";
                    computerOnline = false;
                    computerFileCount = 0;
                    updateDashboard();
                    session.setText("Connect a computer to see its shared files.");
                    postStatus("Pairing removed.");
                });
            });
        }));
        managePairing = disclosure(page, "Manage pairing", saved);
        return page;
    }

    private LinearLayout buildMessagesPage() {
        LinearLayout page = page("Messages");
        LinearLayout main = panel(page, "Conversation with your computer");
        thread = column();
        thread.addView(text("No messages yet.", 14, false));
        main.addView(thread);
        messageInput = input("Write a message");
        LinearLayout compose = panel(page, "New message");
        compose.addView(messageInput);
        compose.addView(button("Send message", true, view -> sendMessage()));
        LinearLayout scratchpad = column();
        EditText snippetInput = input("Add a link or snippet");
        scratchpad.addView(snippetInput);
        scratchpad.addView(button("Share snippet", true, view -> {
            String snippet = snippetInput.getText().toString();
            if (snippet.trim().isEmpty()) return;
            view.setEnabled(false);
            work("Sharing snippet…", () -> {
                try {
                    client.sendSnippet(snippet);
                    runOnUiThread(() -> {
                        if (snippetInput.getText().toString().equals(snippet)) snippetInput.setText("");
                    });
                    refreshScratchpad();
                    postStatus("Snippet shared with the computer.");
                } finally {
                    runOnUiThread(() -> view.setEnabled(true));
                }
            });
        }));
        scratchpadRows = column();
        scratchpad.addView(scratchpadRows);
        scratchpadPanel = disclosure(page, "Scratchpad", scratchpad);
        scratchpadPanel.setVisibility(View.GONE);
        return page;
    }

    private LinearLayout column() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        return layout;
    }

    private LinearLayout page(String title) {
        LinearLayout page = column();
        page.setPadding(0, dp(12), 0, dp(20));
        TextView heading = text("> " + title.toUpperCase(Locale.ROOT), 18, true);
        heading.setAccessibilityHeading(true);
        page.addView(heading);
        return page;
    }

    private LinearLayout panel(LinearLayout parent, String title) {
        LinearLayout panel = column();
        panel.setPadding(dp(16), dp(12), dp(16), dp(16));
        panel.setBackground(border());
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(12);
        parent.addView(panel, params);
        if (!title.isEmpty()) {
            TextView heading = text("> " + title.toUpperCase(Locale.ROOT), 13, true);
            heading.setAccessibilityHeading(true);
            panel.addView(heading);
        }
        return panel;
    }

    private LinearLayout disclosure(LinearLayout parent, String title, LinearLayout content) {
        LinearLayout section = panel(parent, "");
        Button toggle = button(title + "  +", false, view -> {
            boolean open = content.getVisibility() != View.VISIBLE;
            content.setVisibility(open ? View.VISIBLE : View.GONE);
            commandLabel((Button) view, title + (open ? "  −" : "  +"));
        });
        toggle.setGravity(android.view.Gravity.START | android.view.Gravity.CENTER_VERTICAL);
        paintButton(toggle, SURFACE, INK, 0);
        section.addView(toggle, new LinearLayout.LayoutParams(-1, -2));
        section.addView(content);
        content.setVisibility(View.GONE);
        return section;
    }

    private java.util.function.Consumer<Boolean> addSources(LinearLayout parent, LinearLayout controls, LinearLayout computer,
                                                              LinearLayout nearby, String label,
                                                              java.util.function.Consumer<Boolean> onSelected) {
        controls.addView(text(label, 14, false));
        LinearLayout choices = actionRow(controls);
        choices.setPadding(dp(4), dp(4), dp(4), dp(4));
        choices.setBackground(border());
        Button computerChoice = button("Computer", false, null);
        Button phoneChoice = button("Nearby device", false, null);
        choices.addView(computerChoice, new LinearLayout.LayoutParams(0, -2, 1));
        choices.addView(phoneChoice, new LinearLayout.LayoutParams(0, -2, 1));
        parent.addView(computer);
        parent.addView(nearby);
        java.util.function.Consumer<Boolean> select = phone -> {
            computer.setVisibility(phone ? View.GONE : View.VISIBLE);
            nearby.setVisibility(phone ? View.VISIBLE : View.GONE);
            styleChoice(computerChoice, !phone);
            styleChoice(phoneChoice, phone);
            onSelected.accept(phone);
        };
        computerChoice.setOnClickListener(view -> select.accept(false));
        phoneChoice.setOnClickListener(view -> select.accept(true));
        select.accept(false);
        return select;
    }

    private LinearLayout actionRow(LinearLayout parent) {
        LinearLayout row = new LinearLayout(this);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(8);
        parent.addView(row, params);
        return row;
    }

    private Button rowButton(LinearLayout row, String label, boolean filled, View.OnClickListener click) {
        Button button = button(label, filled, click);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, -2, 1);
        if (row.getChildCount() > 0) params.leftMargin = dp(8);
        row.addView(button, params);
        return button;
    }

    private void styleChoice(Button button, boolean selected) {
        button.setSelected(selected);
        paintButton(button, selected ? ACCENT : Color.TRANSPARENT, selected ? BACKGROUND : MUTED, 0);
    }

    private void showDestination(int tab, boolean nearby) {
        if (tab == TAB_SEND) selectSendSource.accept(nearby);
        if (tab == TAB_RECEIVE) selectReceiveSource.accept(nearby);
        selectTab(tab);
    }

    private void selectTab(int tab) {
        if (tab < 0 || tab >= pages.length) return;
        closeConsoleCommand();
        boolean changed = activeTab != tab;
        activeTab = tab;
        for (int i = 0; i < pages.length; i++) {
            pages[i].animate().cancel();
            pages[i].setAlpha(1f);
            pages[i].setTranslationY(0);
            pages[i].setVisibility(i == tab ? View.VISIBLE : View.GONE);
        }
        if (changed && android.animation.ValueAnimator.areAnimatorsEnabled()) {
            View page = pages[tab];
            page.setAlpha(.75f);
            page.setTranslationY(dp(6));
            page.animate().alpha(1f).translationY(0).setDuration(160).start();
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
                    } else showPairingStep("Scan the pairing QR in TapSend on your computer.");
                })
                .addOnFailureListener(error -> showPairingStep("Open pairing on your computer and try the scanner again, or enter the pairing code."));
    }

    private void pair(String raw) {
        final PairingLink link;
        try { link = PairingLink.parse(raw); }
        catch (Exception error) { showPairingStep("On the PC press P, then scan the QR or press Y to copy a fresh pass."); return; }
        pair(link);
    }

    private void pairManual() {
        final PairingLink link;
        try { link = PairingLink.fromManual(addressInput.getText().toString(),
                codeInput.getText().toString(), keyInput.getText().toString()); }
        catch (Exception error) { showPairingStep(detail(error)); return; }
        pair(link);
    }

    private void pair(PairingLink link) {
        String typedName = pairingDeviceName.getText().toString().trim();
        String name = typedName.isEmpty() ? TapSendClient.defaultDeviceName() : typedName;
        if (name.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 80
                || name.codePoints().anyMatch(Character::isISOControl)) {
            showPairingStep("Use a short, single-line name for this phone.");
            return;
        }
        pairingWork("PAIRING", () -> {
            client.pair(link, name);
            getPreferences(MODE_PRIVATE).edit().putString("pairing-name", name).apply();
            client.session();
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

    private void showPairingStep(String message) {
        runOnUiThread(() -> {
            if (destroyed) return;
            pairingNext.setText(message);
            pairingNext.setVisibility(View.VISIBLE);
        });
        postStatus(message);
    }

    private void pairingWork(String label, Job job) {
        pairingNext.setVisibility(View.GONE);
        postStatus(label);
        worker.execute(() -> {
            try { job.run(); }
            catch (Exception error) { showPairingStep(pairingStep(error)); }
        });
    }

    private static String pairingStep(Exception error) {
        for (Throwable cause = error; cause != null; cause = cause.getCause()) {
            if (cause instanceof java.security.cert.CertificateException
                    || cause instanceof javax.net.ssl.SSLHandshakeException) {
                return "The computer’s security key could not be verified. Scan a fresh pairing QR from TapSend.";
            }
            if (cause instanceof java.net.ConnectException) {
                return "Open TapSend on the computer. If it is running, check that Windows Firewall allows TapSend on this network.";
            }
            if (cause instanceof java.net.SocketTimeoutException || cause instanceof java.net.UnknownHostException
                    || cause instanceof java.net.NoRouteToHostException) {
                return "Connect both devices to the same network. If they already are, check Windows Firewall or scan a fresh QR after an IP change.";
            }
        }
        String reason = detail(error);
        String lower = reason.toLowerCase(Locale.ROOT);
        if (lower.contains("waiting for sender approval")) {
            return "On the PC, press V, select this phone, and press A. Then tap Reconnect here.";
        }
        if (lower.contains("pairing code") || lower.contains("unknown device") || lower.contains("http 401")) {
            return "Open pairing in TapSend on the computer, generate a fresh QR, and scan it again.";
        }
        return reason;
    }

    private void showConnected() {
        runOnUiThread(() -> {
            computerOnline = true;
            updateDashboard();
            postStatus("Computer connected.");
            if (activeTab == TAB_CONNECTION && returnToSendAfterPairing) showDestination(TAB_SEND, false);
            else if (activeTab == TAB_CONNECTION) showComputerFiles();
            returnToSendAfterPairing = false;
        });
    }

    private void showComputerFiles() {
        selectReceiveSource.accept(false);
        selectTab(TAB_RECEIVE);
    }

    private void refreshFiles() throws Exception {
        if (!client.hasIdentity()) throw new IllegalStateException("Pair this phone first.");
        JSONObject state = client.session();
        boolean supportsScratchpad = state.optBoolean("scratchpad");
        runOnUiThread(() -> scratchpadPanel.setVisibility(supportsScratchpad ? View.VISIBLE : View.GONE));
        if (supportsScratchpad) {
            try { refreshScratchpad(); } catch (Exception ignored) { }
        }
        boolean targeted = state.optBoolean("targeted", false);
        JSONArray offers = state.optJSONArray("offers");
        boolean hasOffers = offers != null && offers.length() > 0;
        JSONArray files = client.files();
        String catalog = targeted + ":" + files + ":" + offers;
        if (catalog.equals(lastCatalog)) return;
        lastCatalog = catalog;
        targetedShare = targeted;
        runOnUiThread(() -> {
            computerFileCount = files.length();
            updateDashboard();
            if ((hasOffers || (targeted && files.length() > 0)) && activeTab != TAB_SEND && activeTab != TAB_SHARE) {
                showComputerFiles();
            }
            fileList.removeAllViews();
            if (hasOffers) {
                for (int i = 0; i < offers.length(); i++) {
                    JSONObject offer = offers.optJSONObject(i);
                    if (offer != null) addIncomingOffer(offer);
                }
            } else if (files.length() == 0) fileList.addView(text("No files shared yet.", 14, false));
            if (targeted && files.length() > 0) fileList.addView(text("Incoming files for this phone", 14, true));
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
                card.addView(text(String.format(Locale.US, "%.1f MiB", size / 1048576.0), 11, false));
                card.addView(button(targeted ? "Accept and save" : "Save to Downloads", false, view -> download(id, name, size)));
            }
        });
    }

    private void addIncomingOffer(JSONObject offer) {
        JSONArray files = offer.optJSONArray("files");
        if (files == null || files.length() == 0) return;
        String id = offer.optString("id");
        LinearLayout card = panel(fileList, "Incoming offer");
        for (int i = 0; i < files.length(); i++) {
            JSONObject item = files.optJSONObject(i);
            if (item != null) card.addView(text(item.optString("name"), 14, false));
        }
        LinearLayout choices = actionRow(card);
        Button accept = button("Accept", true, null);
        Button decline = button("Decline", false, null);
        choices.addView(accept, new LinearLayout.LayoutParams(0, -2, 1));
        choices.addView(decline, new LinearLayout.LayoutParams(0, -2, 1));
        java.util.function.Consumer<Boolean> answer = accepted -> {
            accept.setEnabled(false);
            decline.setEnabled(false);
            work(accepted ? "Accepting offer…" : "Declining offer…", () -> {
                try {
                    client.answerOffer(id, accepted ? "accepted" : "declined");
                    refreshFiles();
                    postStatus(accepted ? "Offer accepted. Choose a file to save." : "Offer declined.");
                } finally {
                    runOnUiThread(() -> { accept.setEnabled(true); decline.setEnabled(true); });
                }
            });
        };
        accept.setOnClickListener(view -> answer.accept(true));
        decline.setOnClickListener(view -> answer.accept(false));
    }

    private void download(String id, String name, long size) {
        work("Receiving " + name, () -> {
            showCancel(true);
            Uri uri = null;
            try {
                TapSendClient.Progress report = (done, total) -> progress("Receiving " + name, done, total);
                File partial = size >= 32L * 1024 * 1024
                        ? client.stageDownload(id, size, getCacheDir(), report) : null;
                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, name.replace('/', '_').replace('\\', '_'));
                values.put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/TapSend");
                values.put(MediaStore.MediaColumns.IS_PENDING, 1);
                uri = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (uri == null) throw new IllegalStateException("Could not create a Downloads file.");
                try (OutputStream output = getContentResolver().openOutputStream(uri)) {
                    if (output == null) throw new IllegalStateException("Could not write to Downloads.");
                    if (partial == null) {
                        client.download(id, size, output, report);
                    } else {
                        postStatus("Saving " + name + " to Downloads…");
                        try (FileInputStream input = new FileInputStream(partial)) {
                            byte[] buffer = new byte[256 * 1024];
                            int count;
                            while ((count = input.read(buffer)) != -1) {
                                if (client.wasCancelled()) throw new IOException("Download stopped.");
                                output.write(buffer, 0, count);
                            }
                        }
                    }
                }
                values.clear();
                values.put(MediaStore.MediaColumns.IS_PENDING, 0);
                if (getContentResolver().update(uri, values, null, null) != 1) {
                    throw new IllegalStateException("Could not finish saving the Downloads file.");
                }
                if (partial != null) partial.delete();
                client.receipt(id);
                postTransferResult("Saved to Downloads/TapSend: " + name);
            } catch (Exception error) {
                if (uri != null) getContentResolver().delete(uri, null, null);
                client.receipt(id, client.wasCancelled() ? "cancelled" : "failed");
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

    private void choosePortalFiles() {
        if (!client.hasIdentity()) { postStatus("Pair this phone first."); selectTab(TAB_CONNECTION); return; }
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.setType("*/*");
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        startActivityForResult(intent, PICK_PORTAL_FILE);
    }

    private void sendPortalFiles() {
        if (portalSending) return;
        if (portalPending.isEmpty()) { postStatus("Choose portal files first."); return; }
        portalSending = true;
        List<Uri> selected = new ArrayList<>(portalPending);
        portalTile.animate().translationY(-dp(160)).alpha(0f).setDuration(450).withEndAction(() -> {
            portalTile.setVisibility(View.GONE);
            portalTile.setTranslationY(0);
            portalTile.setAlpha(1f);
        }).start();
        uploadSelected(selected, true);
    }

    private void startPeerShare() {
        if (peerServer != null) { postStatus("Phone share is already running."); return; }
        nearbyWork("Starting share…", () -> {
            PeerShareServer created = new PeerShareServer(getContentResolver());
            peerServer = created;
            runOnUiThread(() -> {
                peerShareLink.setText(created.url());
                showPeerQr(created.url());
                updateSharePanel(created);
            });
            postStatus("Share ready. Add files and copy the link.");
        });
    }

    private void stopPeerShare() {
        PeerShareServer old = peerServer;
        peerServer = null;
        if (old != null) old.close();
        peerShareLink.setText("");
        peerShareDetails.setVisibility(View.GONE);
        startShareButton.setVisibility(View.VISIBLE);
        peerShareQr.setImageDrawable(null);
        peerShareQr.setVisibility(View.GONE);
        peerShareCount.setText("No files shared yet");
        peerJoinSummary.setText("No receivers yet");
        peerJoinRequests.removeAllViews();
        peerTransferSummary.setText("No files sent yet");
        lastPeerJoins = "";
        postStatus("Phone share stopped. The old link no longer works.");
    }

    private void updateSharePanel(PeerShareServer share) {
        peerShareDetails.setVisibility(View.VISIBLE);
        startShareButton.setVisibility(View.GONE);
        int count = share.count();
        peerShareCount.setText(count == 1 ? "1 file shared" : count + " files shared");
        updatePeerJoinRequests();
    }

    private void updatePeerJoinRequests() {
        PeerShareServer current = peerServer;
        if (current == null) return;
        String transfers = current.transferSummary();
        if (!transfers.contentEquals(peerTransferSummary.getText())) peerTransferSummary.setText(transfers);
        List<String> pending = current.pending();
        String signature = current.joinedCount() + ":" + String.join(",", pending);
        if (signature.equals(lastPeerJoins)) return;
        lastPeerJoins = signature;
        peerJoinSummary.setText(current.joinedCount() + " connected · " + pending.size() + " waiting");
        peerJoinRequests.removeAllViews();
        for (String address : pending) {
            peerJoinRequests.addView(text("Join request · " + address, 14, true));
            LinearLayout actions = actionRow(peerJoinRequests);
            rowButton(actions, "Approve", true, view -> {
                current.approve(address);
                updatePeerJoinRequests();
                postStatus("Approved " + address);
            });
            rowButton(actions, "Deny", false, view -> {
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
        nearbyWork("Connecting to phone…", () -> refreshPeerFiles(incoming));
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
            if (files.length() == 0) peerFileList.addView(text("No files shared yet.", 14, false));
            for (int i = 0; i < files.length(); i++) {
                JSONObject item = files.optJSONObject(i);
                if (item == null) continue;
                int id = item.optInt("id", -1);
                String name = item.optString("name", "file");
                long size = item.optLong("size", -1);
                Button save = button("Save " + name, false, view -> downloadPeerFile(incoming, id, name, size));
                save.setSingleLine(false);
                peerFileList.addView(save);
            }
        });
        postStatus("Connected to phone. Tap a file to save it.");
    }

    private void downloadPeerFile(PeerShareClient incoming, int id, String name, long size) {
        nearbyWork("Receiving " + name, () -> {
            Uri destination = null;
            try {
                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, name.replace('/', '_').replace('\\', '_'));
                values.put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/TapSend");
                values.put(MediaStore.MediaColumns.IS_PENDING, 1);
                destination = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (destination == null) throw new IllegalStateException("Could not create a Downloads file.");
                long transfer;
                try (OutputStream output = getContentResolver().openOutputStream(destination)) {
                    if (output == null) throw new IllegalStateException("Could not write to Downloads.");
                    transfer = incoming.download(id, size, output, done -> progress("Receiving " + name, done, size));
                }
                values.clear();
                values.put(MediaStore.MediaColumns.IS_PENDING, 0);
                if (getContentResolver().update(destination, values, null, null) != 1)
                    throw new IllegalStateException("Could not finish saving the file.");
                Uri saved = destination;
                destination = null;
                String savedName = name;
                try (Cursor cursor = getContentResolver().query(saved,
                        new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
                    if (cursor != null && cursor.moveToFirst() && !cursor.isNull(0)) {
                        savedName = cursor.getString(0);
                    }
                } catch (RuntimeException ignored) { }
                postTransferResult(savedName.equals(name) ? "Saved to Downloads/TapSend: " + name
                        : name + " was saved as " + savedName + " in Downloads/TapSend.");
                incoming.confirmSaved(id, transfer);
            } catch (Exception error) {
                if (destination != null) getContentResolver().delete(destination, null, null);
                throw error;
            }
        });
    }

    @Override protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == SCREEN_CONSENT || requestCode == SCREEN_PEN_CONSENT) {
            if (resultCode == RESULT_OK && data != null) {
                startForegroundService(new Intent(this, ScreenCaptureService.class)
                        .putExtra(ScreenCaptureService.CONSENT, data)
                        .putExtra(ScreenCaptureService.RED_PEN, requestCode == SCREEN_PEN_CONSENT));
                postStatus("Connecting screen sharing to Windows…");
            } else postStatus("Screen sharing cancelled.");
            return;
        }
        if ((requestCode != PICK_FILE && requestCode != PICK_PEER_FILE && requestCode != PICK_PORTAL_FILE) || resultCode != RESULT_OK || data == null) return;
        List<Uri> selected = new ArrayList<>();
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int i = 0; i < clip.getItemCount(); i++) selected.add(clip.getItemAt(i).getUri());
        } else if (data.getData() != null) selected.add(data.getData());
        selected.removeIf(java.util.Objects::isNull);
        if (selected.isEmpty()) return;
        if (requestCode == PICK_PORTAL_FILE) {
            portalPending.clear();
            portalPending.addAll(selected);
            if (!selected.isEmpty()) {
                portalTile.setText(selected.size() == 1 ? "Tap or swipe up to send\n" + selected.get(0).getLastPathSegment() : selected.size() + " files ready to send");
                portalTile.setVisibility(View.VISIBLE);
                postStatus("Tap or swipe the card up to send.");
            }
            return;
        }
        if (requestCode == PICK_PEER_FILE) {
            shareNearby(selected, false);
            return;
        }
        addToQueue(sendQueue, selected);
        renderSendQueue();
        postStatus(selected.size() == 1 ? "File added to the queue." : selected.size() + " files added to the queue.");
    }

    private void shareNearby(List<Uri> selected, boolean fromShareMenu) {
        if (preparingNearby) return;
        preparingNearby = true;
        renderNearbyQueue();
        nearbyWork("Preparing files to share…", () -> {
            try {
                List<PeerShareServer.File> files = new ArrayList<>();
                for (Uri uri : selected) files.add(readFile(uri));
                PeerShareServer active = peerServer;
                if (active == null) {
                    active = new PeerShareServer(getContentResolver());
                    peerServer = active;
                }
                if (destroyed) {
                    active.close();
                    return;
                }
                for (PeerShareServer.File file : files) active.add(file);
                PeerShareServer share = active;
                runOnUiThread(() -> {
                    if (destroyed || peerServer != share) return;
                    if (fromShareMenu) nearbyQueue.removeAll(selected);
                    peerShareLink.setText(share.url());
                    showPeerQr(share.url());
                    updateSharePanel(share);
                    postStatus("Share ready. Show the QR or copy the link.");
                });
            } finally {
                runOnUiThread(() -> {
                    preparingNearby = false;
                    if (!destroyed) renderNearbyQueue();
                });
            }
        });
    }

    private PeerShareServer.File readFile(Uri uri) {
        String name = "file";
        long size = -1;
        try (Cursor cursor = getContentResolver().query(uri,
                new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                if (!cursor.isNull(0)) name = cursor.getString(0);
                if (!cursor.isNull(1)) size = cursor.getLong(1);
            }
        }
        return new PeerShareServer.File(uri, name, size);
    }

    private void renderSendQueue() {
        renderFileSelection(selectedFiles, sendQueue, queueSending, this::renderSendQueue);
        sendQueueButton.setVisibility(sendQueue.isEmpty() ? View.GONE : View.VISIBLE);
        sendQueueButton.setEnabled(!queueSending);
        commandLabel(sendQueueButton, queueSending ? "Sending…" : sendQueue.size() == 1 ? "Send 1 file" : "Send " + sendQueue.size() + " files");
    }

    private void renderFileSelection(LinearLayout container, List<Uri> queue, boolean locked, Runnable refresh) {
        container.removeAllViews();
        for (Uri uri : List.copyOf(queue)) {
            String label;
            try {
                PeerShareServer.File file = readFile(uri);
                label = file.name + (file.size < 0 ? "" : String.format(Locale.US, "\n%.2f MiB", file.size / 1048576.0));
            } catch (RuntimeException error) {
                label = "Unavailable file";
            }
            android.widget.CheckBox item = new android.widget.CheckBox(this);
            item.setText(label);
            item.setTextSize(13);
            item.setTextColor(INK);
            item.setButtonTintList(android.content.res.ColorStateList.valueOf(ACCENT));
            item.setMinHeight(dp(52));
            item.setEnabled(!locked);
            item.setChecked(true);
            item.setOnCheckedChangeListener((button, checked) -> {
                if (!checked) {
                    queue.remove(uri);
                    refresh.run();
                }
            });
            container.addView(item);
        }
    }

    private void uploadSelected(List<Uri> selected) {
        uploadSelected(selected, false);
    }

    private void uploadSelected(List<Uri> selected, boolean portalMode) {
        work("Checking computer destination…", () -> {
            try {
                JSONObject drop = client.folderDrop();
                String folder = drop.optString("folder"), token = drop.optString("token");
                if (folder.isEmpty() || token.isEmpty()) {
                    uploadToComputer(selected, portalMode, "");
                    return;
                }
                runOnUiThread(() -> {
                    if (destroyed) return;
                    new android.app.AlertDialog.Builder(this)
                            .setTitle(">_  DROP TO FOLDER")
                            .setMessage(folder + "\n\nExisting files are kept. Duplicates get a numbered name.")
                            .setPositiveButton("Send here", (dialog, which) -> uploadToComputer(selected, portalMode, token))
                            .setNeutralButton("Use inbox", (dialog, which) -> uploadToComputer(selected, portalMode, ""))
                            .setNegativeButton("Cancel", (dialog, which) -> finishUpload(portalMode, false))
                            .setOnCancelListener(dialog -> finishUpload(portalMode, false))
                            .show();
                });
            } catch (Exception error) {
                runOnUiThread(() -> finishUpload(portalMode, false));
                throw error;
            }
        });
    }

    private void uploadToComputer(List<Uri> selected, boolean portalMode, String folderToken) {
        work("Preparing files…", () -> {
            showCancel(true);
            boolean succeeded = false;
            try {
                for (Uri uri : selected) {
                    PeerShareServer.File file = readFile(uri);
                    JSONObject receipt = client.upload(file.name, file.size, folderToken,
                            () -> getContentResolver().openInputStream(uri),
                            (done, total) -> progress("Sending " + file.name, done, total));
                    String savedName = receipt.optString("name", file.name);
                    String folder = receipt.optString("folder");
                    postTransferResult((savedName.equals(file.name) ? "Saved on computer: " : "Original kept. Saved as ") +
                            savedName + (folder.isEmpty() ? "" : "\n" + folder));
                    runOnUiThread(() -> {
                        if (!portalMode) sendQueue.remove(uri);
                        if (uploadHistory.getChildCount() == 1 &&
                                ((TextView) uploadHistory.getChildAt(0)).getText().toString().equals("No files sent to the computer yet.")) {
                            uploadHistory.removeAllViews();
                        }
                        uploadHistory.addView(text(savedName, 14, true));
                    });
                }
                succeeded = true;
            } finally {
                showCancel(false);
                boolean completed = succeeded;
                runOnUiThread(() -> finishUpload(portalMode, completed));
            }
        });
    }

    private void finishUpload(boolean portalMode, boolean completed) {
        if (destroyed) return;
        if (!portalMode) {
            queueSending = false;
            renderSendQueue();
            return;
        }
        portalSending = false;
        if (completed) portalPending.clear();
        else {
            portalTile.animate().cancel();
            portalTile.setTranslationY(0);
            portalTile.setAlpha(1f);
            portalTile.setVisibility(View.VISIBLE);
        }
    }

    private void sendMessage() {
        if (!client.hasIdentity()) { postStatus("Pair this phone first."); selectTab(TAB_CONNECTION); return; }
        String text = messageInput.getText().toString().trim();
        if (text.isEmpty()) return;
        work("Sending message…", () -> {
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
                    runOnUiThread(() -> { computerOnline = true; updateDashboard(); });
                    runOnUiThread(() -> session.setText(stopped ? "Sharing paused on the computer." : targetedShare ? "Incoming files for this phone" : "Connected"));
                    refreshMessages();
                    refreshFiles();
                } catch (Exception error) {
                    runOnUiThread(() -> { computerOnline = false; updateDashboard(); session.setText("Connection lost. " + detail(error)); });
                }
            }
            if (!destroyed) handler.postDelayed(this::poll, 3000);
        });
    }

    private void pollNearby() {
        if (destroyed) return;
        updatePeerJoinRequests();
        updateDashboard();
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
            if (messages.length() == 0) thread.addView(text("No messages yet.", 12, false));
            for (int i = 0; i < messages.length(); i++) {
                JSONObject item = messages.optJSONObject(i);
                if (item == null) continue;
                String from = item.optString("from").equals("phone") ? "You" : "Computer";
                TextView line = text(from + "\n" + item.optString("text"), 14, true);
                line.setPadding(dp(8), dp(8), dp(8), dp(8));
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
                params.topMargin = dp(6);
                line.setBackground(border());
                thread.addView(line, params);
            }
        });
    }

    private void refreshScratchpad() throws Exception {
        JSONArray snippets = client.snippets();
        String snapshot = snippets.toString();
        if (scratchpadSnapshot.equals(snapshot)) return;
        scratchpadSnapshot = snapshot;
        runOnUiThread(() -> {
            scratchpadRows.removeAllViews();
            if (snippets.length() == 0) scratchpadRows.addView(text("No snippets yet.", 14, false));
            for (int i = snippets.length() - 1; i >= 0; i--) {
                JSONObject item = snippets.optJSONObject(i);
                if (item == null) continue;
                String content = item.optString("text");
                LinearLayout row = panel(scratchpadRows, item.optString("from").equals("phone") ? "You" : "Computer");
                row.addView(text(content, 14, false));
                row.addView(button("Copy snippet", false, view -> {
                    ClipboardManager clipboard = getSystemService(ClipboardManager.class);
                    clipboard.setPrimaryClip(ClipData.newPlainText("TapSend snippet", content));
                    postStatus("Snippet copied.");
                }));
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

    private synchronized void progress(String label, long done, long total) {
        long now = android.os.SystemClock.elapsedRealtime();
        if (!label.equals(sampleLabel) || done < sampleBytes) {
            sampleLabel = label;
            sampleTime = now;
            sampleBytes = 0;
            lastProgressUi = 0;
        }
        if (now - lastProgressUi < 200 && done != total) return;
        double speed = (done - sampleBytes) * 1000.0 / Math.max(1, now - sampleTime) / 1048576.0;
        sampleBytes = done;
        sampleTime = now;
        lastProgressUi = now;
        int percent = total > 0 ? (int) Math.min(100, done * 100 / total) : 0;
        String amount = total > 0 ? percent + "%" : String.format(Locale.US, "%.1f MiB", done / 1048576.0);
        String value = label + "\n" + amount + String.format(Locale.US, " · %.2f MiB/s", speed);
        runOnUiThread(() -> {
            statusPanel.setVisibility(View.VISIBLE);
            status.setText(value);
            transferProgress.setVisibility(View.VISIBLE);
            transferProgress.setIndeterminate(total < 0);
            if (total >= 0) transferProgress.setProgress(percent);
        });
    }

    private void showCancel(boolean visible) {
        runOnUiThread(() -> cancelButton.setVisibility(visible ? View.VISIBLE : View.GONE));
    }

    private void postStatus(String value) {
        runOnUiThread(() -> {
            if (destroyed) return;
            statusPanel.setVisibility(View.VISIBLE);
            status.setText(value);
            transferProgress.setVisibility(View.GONE);
            eventLog.addFirst(value);
            while (eventLog.size() > 12) eventLog.removeLast();
            dashboardLogs.setText(String.join("\n\n", eventLog));
            updateDashboard();
        });
    }

    private LinearLayout buildDashboardPage() {
        LinearLayout page = column();
        page.setPadding(0, dp(14), 0, dp(20));
        TextView heading = text("Transfer session", 22, true);
        heading.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
        heading.setAccessibilityHeading(true);
        page.addView(heading);
        addRule(page);

        LinearLayout devices = new LinearLayout(this);
        computerTab = consoleDeviceTab(devices, "Computer", false);
        nearbyTab = consoleDeviceTab(devices, "Nearby phone", true);
        page.addView(devices, new LinearLayout.LayoutParams(-1, -2));

        LinearLayout diagram = column();
        diagram.setPadding(dp(12), dp(15), dp(12), dp(10));
        diagram.setBackground(new DotGrid());
        LinearLayout endpoints = new LinearLayout(this);
        endpoints.setGravity(android.view.Gravity.TOP);

        LinearLayout local = column();
        local.setGravity(android.view.Gravity.CENTER_HORIZONTAL);
        DeviceGlyph phone = new DeviceGlyph(false, false);
        local.addView(phone, new LinearLayout.LayoutParams(dp(50), dp(50)));
        TextView source = text("This phone", 11, true);
        source.setGravity(android.view.Gravity.CENTER);
        local.addView(source, new LinearLayout.LayoutParams(-1, -2));
        endpoints.addView(local, new LinearLayout.LayoutParams(0, -2, 1));

        View link = new View(this) {
            private final Paint line = new Paint(Paint.ANTI_ALIAS_FLAG);
            @Override protected void onDraw(Canvas canvas) {
                float y = getHeight() / 2f;
                float middle = getWidth() / 2f;
                float gap = dp(10);
                line.setColor(MUTED);
                line.setStrokeWidth(dp(1));
                line.setPathEffect(new android.graphics.DashPathEffect(new float[]{dp(3), dp(3)}, 0));
                canvas.drawLine(0, y, Math.max(0, middle - gap), y, line);
                canvas.drawLine(Math.min(getWidth(), middle + gap), y, getWidth(), y, line);
                line.setPathEffect(null);
                line.setColor(INK);
                line.setTypeface(Typeface.MONOSPACE);
                line.setTextSize(dp(15));
                line.setTextAlign(Paint.Align.CENTER);
                canvas.drawText("⇄", middle, y - (line.ascent() + line.descent()) / 2f, line);
            }
        };
        link.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        endpoints.addView(link, new LinearLayout.LayoutParams(0, dp(50), .9f));

        LinearLayout remote = column();
        remote.setGravity(android.view.Gravity.CENTER_HORIZONTAL);
        destinationIcon = new DeviceGlyph(true, true);
        remote.addView(destinationIcon, new LinearLayout.LayoutParams(dp(50), dp(50)));
        consoleDestination = text("My computer", 11, true);
        consoleDestination.setGravity(android.view.Gravity.CENTER);
        remote.addView(consoleDestination, new LinearLayout.LayoutParams(-1, -2));
        endpoints.addView(remote, new LinearLayout.LayoutParams(0, -2, 1));

        diagram.addView(endpoints, new LinearLayout.LayoutParams(-1, -2));
        consoleStatus = text("", 11, false);
        consoleStatus.setGravity(android.view.Gravity.CENTER);
        consoleStatus.setPadding(dp(7), dp(3), dp(7), dp(3));
        consoleStatus.setBackground(shape(BACKGROUND, 0xff454545));
        LinearLayout.LayoutParams statusParams = new LinearLayout.LayoutParams(-2, -2);
        statusParams.gravity = android.view.Gravity.CENTER_HORIZONTAL;
        diagram.addView(consoleStatus, statusParams);
        page.addView(diagram, new LinearLayout.LayoutParams(-1, -2));
        addRule(page);

        sendCommand = consoleCommand(page, "↑", TAB_SEND);
        receiveCommand = consoleCommand(page, "↓", TAB_RECEIVE);
        consoleWork = column();
        consoleWork.setVisibility(View.GONE);
        page.addView(consoleWork, new LinearLayout.LayoutParams(-1, -2));

        TextView activityTitle = text("Transfer activity", 14, true);
        activityTitle.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
        activityTitle.setAccessibilityHeading(true);
        LinearLayout.LayoutParams activityParams = new LinearLayout.LayoutParams(-1, -2);
        activityParams.topMargin = dp(14);
        page.addView(activityTitle, activityParams);

        LinearLayout activity = column();
        activity.setPadding(dp(10), dp(4), dp(10), dp(4));
        activity.setBackground(shape(SURFACE, 0xff454545));
        page.addView(activity);
        LinearLayout receipt = new LinearLayout(this);
        receipt.setGravity(android.view.Gravity.CENTER_VERTICAL);
        receipt.setMinimumHeight(dp(48));
        receipt.setFocusable(true);
        receiptMark = text("›", 16, true);
        receiptMark.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        receipt.addView(receiptMark, new LinearLayout.LayoutParams(dp(25), -2));
        latestTransfer = text("No completed transfers this session.", 12, false);
        receipt.addView(latestTransfer, new LinearLayout.LayoutParams(0, -2, 1));
        TextView expand = text("↗", 12, false);
        expand.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        receipt.addView(expand);
        activity.addView(receipt, new LinearLayout.LayoutParams(-1, -2));
        dashboardLogs = text("No recent activity.", 12, false);
        dashboardLogs.setTextIsSelectable(true);
        dashboardLogs.setVisibility(View.GONE);
        receipt.setOnClickListener(view -> {
            boolean open = dashboardLogs.getVisibility() != View.VISIBLE;
            dashboardLogs.setVisibility(open ? View.VISIBLE : View.GONE);
            expand.setText(open ? "⌄" : "↗");
        });
        activity.addView(dashboardLogs);
        return page;
    }

    private void addRule(LinearLayout layout) {
        View rule = new View(this);
        rule.setBackgroundColor(0xff3b3b3b);
        layout.addView(rule, new LinearLayout.LayoutParams(-1, dp(1)));
    }

    private Button consoleDeviceTab(LinearLayout row, String label, boolean nearby) {
        Button tab = button(label, false, view -> {
            consoleNearby = nearby;
            updateConsole();
            if (consolePage == TAB_SEND) selectSendSource.accept(nearby);
            if (consolePage == TAB_RECEIVE) selectReceiveSource.accept(nearby);
        });
        tab.setText(label);
        row.addView(tab, new LinearLayout.LayoutParams(0, -2, 1));
        return tab;
    }

    private View consoleCommand(LinearLayout page, String arrow, int tab) {
        boolean sending = tab == TAB_SEND;
        LinearLayout command = new LinearLayout(this);
        command.setGravity(android.view.Gravity.CENTER_VERTICAL);
        command.setMinimumHeight(dp(58));
        command.setPadding(dp(9), dp(6), dp(9), dp(6));
        command.setFocusable(true);
        command.setOnClickListener(view -> toggleConsoleCommand(tab));
        TextView icon = text(arrow, 18, true);
        icon.setGravity(android.view.Gravity.CENTER);
        icon.setTextColor(sending ? BACKGROUND : INK);
        icon.setBackgroundColor(sending ? INK : 0xff292929);
        icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        command.addView(icon, new LinearLayout.LayoutParams(dp(35), dp(35)));
        LinearLayout labels = column();
        LinearLayout.LayoutParams labelParams = new LinearLayout.LayoutParams(0, -2, 1);
        labelParams.leftMargin = dp(9);
        labelParams.rightMargin = dp(6);
        TextView title = text("", 13, true);
        TextView detail = text("", 11, false);
        title.setPadding(0, 0, 0, 0);
        detail.setPadding(0, 0, 0, 0);
        labels.addView(title);
        labels.addView(detail);
        command.addView(labels, labelParams);
        TextView enter = text("↵", 12, false);
        enter.setGravity(android.view.Gravity.CENTER);
        enter.setBackground(shape(SURFACE, 0xff5b5b5b));
        enter.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        command.addView(enter, new LinearLayout.LayoutParams(dp(21), dp(27)));
        if (sending) {
            sendCommandTitle = title;
            sendCommandDetail = detail;
        } else {
            receiveCommandTitle = title;
            receiveCommandDetail = detail;
        }
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(sending ? 12 : 6);
        page.addView(command, params);
        return command;
    }

    private void toggleConsoleCommand(int tab) {
        if (consolePage == tab) {
            closeConsoleCommand();
            return;
        }
        if (!consoleNearby && !client.hasIdentity()) {
            returnToSendAfterPairing = tab == TAB_SEND;
            selectTab(TAB_CONNECTION);
            return;
        }
        closeConsoleCommand();
        if (tab == TAB_SEND) selectSendSource.accept(consoleNearby);
        else selectReceiveSource.accept(consoleNearby);
        LinearLayout page = pages[tab];
        pageContent.removeView(page);
        consoleWork.addView(page, new LinearLayout.LayoutParams(-1, -2));
        consolePage = tab;
        setConsoleChrome(tab, true);
        page.setVisibility(View.VISIBLE);
        consoleWork.setVisibility(View.VISIBLE);
        updateCommandOutline();
        consoleWork.post(() -> {
            if (consolePage == tab) {
                consoleWork.requestRectangleOnScreen(new android.graphics.Rect(
                        0, 0, consoleWork.getWidth(), Math.min(dp(110), consoleWork.getHeight())));
            }
        });
    }

    private void closeConsoleCommand() {
        if (consolePage == -1) return;
        LinearLayout embedded = pages[consolePage];
        consoleWork.removeView(embedded);
        embedded.setVisibility(View.GONE);
        pageContent.addView(embedded);
        setConsoleChrome(consolePage, false);
        consolePage = -1;
        consoleWork.setVisibility(View.GONE);
        updateCommandOutline();
    }

    private void setConsoleChrome(int tab, boolean inline) {
        LinearLayout page = pages[tab];
        page.getChildAt(0).setVisibility(inline ? View.GONE : View.VISIBLE);
        (tab == TAB_SEND ? sendSourceControls : receiveSourceControls).setVisibility(inline ? View.GONE : View.VISIBLE);
        page.setPadding(0, inline ? 0 : dp(12), 0, inline ? 0 : dp(20));
    }

    private void updateCommandOutline() {
        for (int tab : new int[]{TAB_SEND, TAB_RECEIVE}) {
            View view = tab == TAB_SEND ? sendCommand : receiveCommand;
            boolean expanded = consolePage == tab;
            view.setSelected(expanded);
            view.setBackground(new android.graphics.drawable.RippleDrawable(
                    android.content.res.ColorStateList.valueOf(0x28ffffff),
                    shape(expanded ? 0xff202020 : 0xff161616, expanded ? 0xffb0b0b0 : 0xff373737),
                    shape(Color.WHITE, 0)));
        }
    }

    private void updateConsole() {
        boolean paired = client.hasIdentity();
        PeerShareServer share = peerServer;
        consoleDestination.setText(consoleNearby ? "Nearby phone" : "My computer");
        destinationIcon.setMonitor(!consoleNearby);
        styleConsoleTab(computerTab, !consoleNearby);
        styleConsoleTab(nearbyTab, consoleNearby);
        String count = computerFileCount == 1 ? "1 shared file" : computerFileCount + " shared files";
        consoleStatus.setText(consoleNearby
                ? share == null ? "Same Wi-Fi · QR or share link" : "● Share active · " + share.count() + (share.count() == 1 ? " file" : " files")
                : computerOnline ? "● Connected · " + count : paired ? "Computer offline" : "Computer not paired");
        sendCommandTitle.setText(consoleNearby ? "Share files" : "Send files");
        sendCommandDetail.setText(consoleNearby ? "Create a share QR and link" : "Choose files on this phone");
        receiveCommandTitle.setText(consoleNearby ? "Scan or paste link" : "View shared files");
        receiveCommandDetail.setText(consoleNearby ? "Open a nearby phone share" : "Save from your computer");
    }

    private void styleConsoleTab(Button tab, boolean selected) {
        tab.setSelected(selected);
        tab.setTextColor(selected ? INK : MUTED);
        tab.setBackground(new android.graphics.drawable.RippleDrawable(
                android.content.res.ColorStateList.valueOf(0x28ffffff),
                selected ? new android.graphics.drawable.LayerDrawable(new android.graphics.drawable.Drawable[]{
                        shape(INK, 0), new android.graphics.drawable.InsetDrawable(shape(0xff181818, 0), 0, 0, 0, dp(2))
                }) : shape(BACKGROUND, 0),
                shape(Color.WHITE, 0)));
    }

    private void updateDashboard() {
        boolean paired = client.hasIdentity();
        updateConsole();
        commandLabel(confirmNearby, peerServer == null ? "Create QR and link" : "Add files to active share");
        connection.setText(computerOnline ? "Connected · " + client.host()
                : paired ? "Paired · currently offline" : "No computer paired");
        reconnectButton.setVisibility(paired && !computerOnline ? View.VISIBLE : View.GONE);
        managePairing.setVisibility(paired ? View.VISIBLE : View.GONE);
    }

    private final class DotGrid extends android.graphics.drawable.Drawable {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);

        @Override public void draw(Canvas canvas) {
            android.graphics.Rect bounds = getBounds();
            paint.setColor(0xff101010);
            canvas.drawRect(bounds, paint);
            paint.setColor(0xff3a3a3a);
            int step = dp(8);
            for (int x = bounds.left + step / 2; x < bounds.right; x += step) {
                for (int y = bounds.top + step / 2; y < bounds.bottom; y += step) {
                    canvas.drawCircle(x, y, Math.max(1f, dp(1)), paint);
                }
            }
        }

        @Override public void setAlpha(int alpha) { }
        @Override public void setColorFilter(android.graphics.ColorFilter filter) { }
        @Override public int getOpacity() { return android.graphics.PixelFormat.OPAQUE; }
    }

    private final class DeviceGlyph extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private boolean monitor;

        DeviceGlyph(boolean highlighted, boolean monitor) {
            super(MainActivity.this);
            this.monitor = monitor;
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
            setBackground(shape(highlighted ? INK : 0xff1c1c1c, highlighted ? INK : 0xff717171));
            paint.setColor(highlighted ? BACKGROUND : INK);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(dp(1));
        }

        void setMonitor(boolean monitor) {
            if (this.monitor == monitor) return;
            this.monitor = monitor;
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            float x = getWidth() / 2f;
            float y = getHeight() / 2f;
            if (monitor) {
                canvas.drawRect(x - dp(8), y - dp(6), x + dp(8), y + dp(3), paint);
                canvas.drawLine(x, y + dp(3), x, y + dp(7), paint);
                canvas.drawLine(x - dp(4), y + dp(7), x + dp(4), y + dp(7), paint);
            } else {
                canvas.drawRect(x - dp(5), y - dp(8), x + dp(5), y + dp(8), paint);
                canvas.drawLine(x - dp(1), y + dp(5), x + dp(1), y + dp(5), paint);
            }
        }
    }

    private void postTransferResult(String result) {
        runOnUiThread(() -> {
            if (destroyed) return;
            receiptMark.setText("✓");
            latestTransfer.setText(result);
            latestTransfer.setTextColor(INK);
            postStatus(result);
        });
    }

    private static String detail(Exception error) {
        String message = error.getMessage();
        return message == null || message.isBlank() ? error.getClass().getSimpleName() : message;
    }

    private TextView text(String value, int size, boolean bold) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextColor(bold ? INK : MUTED);
        view.setTextSize(size);
        view.setTypeface(getResources().getFont(R.font.jetbrains_mono), bold ? Typeface.BOLD : Typeface.NORMAL);
        view.setPadding(0, dp(4), 0, dp(4));
        return view;
    }

    private EditText input(String hint) {
        EditText view = new EditText(this);
        view.setMinLines(1);
        view.setMaxLines(3);
        view.setHint(hint);
        view.setHintTextColor(MUTED);
        view.setMinimumHeight(dp(48));
        view.setTextColor(INK);
        view.setTextSize(15);
        view.setPadding(dp(12), dp(10), dp(12), dp(10));
        view.setBackground(border());
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(8);
        view.setLayoutParams(params);
        return view;
    }

    private Button button(String label, boolean filled, View.OnClickListener click) {
        Button view = new Button(this);
        view.setAllCaps(false);
        commandLabel(view, label);
        view.setTextSize(12);
        view.setTypeface(getResources().getFont(R.font.jetbrains_mono), Typeface.BOLD);
        view.setMinimumWidth(0);
        view.setMinWidth(0);
        view.setMinimumHeight(dp(48));
        view.setMinHeight(dp(48));
        view.setPadding(dp(12), dp(8), dp(12), dp(8));
        paintButton(view, filled ? ACCENT : SURFACE, filled ? BACKGROUND : INK, filled ? 0 : BORDER);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(8);
        view.setLayoutParams(params);
        view.setOnClickListener(click);
        return view;
    }

    private static void commandLabel(Button button, String label) {
        button.setText("[ " + label.toUpperCase(Locale.ROOT) + " ]");
        button.setContentDescription(label);
    }

    private void paintButton(Button view, int fill, int ink, int stroke) {
        view.setTextColor(new android.content.res.ColorStateList(
                new int[][] {new int[] {-android.R.attr.state_enabled}, new int[0]},
                new int[] {0xff808080, ink}));
        view.setBackground(new android.graphics.drawable.RippleDrawable(
                android.content.res.ColorStateList.valueOf(0x33ffffff),
                shape(fill, stroke), shape(Color.WHITE, 0)));
    }

    private GradientDrawable border() {
        return shape(SURFACE, BORDER);
    }

    private GradientDrawable shape(int color, int stroke) {
        GradientDrawable shape = new GradientDrawable();
        shape.setColor(color);
        if (stroke != 0) shape.setStroke(dp(1), stroke);
        return shape;
    }

    private int dp(int value) {
        return Math.round(getResources().getDisplayMetrics().density * value);
    }

    private interface Job { void run() throws Exception; }
}
