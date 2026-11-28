from html import escape


def flow(title, *steps):
    items = "".join(
        f"<li><strong>{escape(label)}</strong><span>{escape(detail)}</span></li>"
        for label, detail in steps
    )
    return f"""
        <figure class="guide-visual">
          <figcaption>{escape(title)}</figcaption>
          <ol class="visual-flow">{items}</ol>
        </figure>
    """


def readout(title, *rows):
    entries = "".join(
        f"<div><dt>{escape(label)}</dt><dd>{escape(detail)}</dd></div>"
        for label, detail in rows
    )
    return f"""
        <figure class="guide-visual visual-readout">
          <figcaption>{escape(title)}</figcaption>
          <dl>{entries}</dl>
        </figure>
    """


def diagram(title, name, height, description):
    return f"""
        <figure class="guide-visual guide-diagram">
          <figcaption>{escape(title)}</figcaption>
          <img src="assets/{name}.svg" width="600" height="{height}" alt="{escape(description)}">
        </figure>
    """


def screen_view(mode):
    title, status, shortcut = {
        "live": ("Android screen on Windows", "LIVE", "Esc ends the viewer"),
        "freeze": ("Select part of a frozen frame", "FROZEN", "G saves an image · T reads text"),
        "pen": ("Point to a control from Windows", "LIVE RED PEN", "Right-drag to send an arrow"),
    }[mode]
    return f"""
        <figure class="guide-visual screen-visual screen-{mode}">
          <figcaption><span>{title}</span><span class="visual-state">{status}</span></figcaption>
          <div class="viewer-stage">
            <div class="viewer-phone">
              <div class="viewer-phone-top">10:42 <span aria-hidden="true">▮▮▮</span></div>
              <div class="viewer-phone-app">
                <strong>Daily checklist</strong>
                <div class="viewer-checks">
                  <span>✓ Capture photos</span><span>✓ Pick files</span><span>□ Send to PC</span>
                </div>
                <span class="viewer-selection" aria-hidden="true"></span>
                <span class="viewer-arrow" aria-hidden="true">↖</span>
              </div>
            </div>
          </div>
          <div class="visual-foot">{shortcut}</div>
        </figure>
    """


def desktop_preview():
    return """
      <figure class="terminal-trace desktop-preview">
        <div class="trace-title"><span class="trace-icon" aria-hidden="true">&gt;_</span><figcaption>TapSend desktop preview</figcaption><span class="trace-index">● LAN ready</span></div>
        <div class="trace-content">
          <div class="devices"><div><span class="display-icon" aria-hidden="true"></span>Windows PC</div><span class="connection" aria-hidden="true"><b>⇄</b></span><div><span class="phone-icon" aria-hidden="true"></span>Pixel 8</div></div>
          <div class="preview-queue">
            <div class="preview-queue-title"><strong>Transfer queue</strong><span>2 files</span></div>
            <div><span>session-notes.txt</span><strong>✓ Saved</strong></div>
            <div><span>field-footage.mp4</span><span>68% · Receiving</span></div>
            <progress aria-label="Example transfer progress" max="100" value="68">68%</progress>
          </div>
        </div>
        <div class="trace-footer"><kbd>V</kbd> Devices <kbd>S</kbd> Send <kbd>N</kbd> Help</div>
      </figure>
    """


GUIDE_VISUALS = {
    "quick-start": flow(
        "From install to first transfer",
        ("Open TapSend", "Finish setup on the PC"),
        ("Pair a phone", "Show the Windows QR with P and scan it"),
        ("Send a file", "Choose a phone, mark a file, then send"),
    ),
    "features": readout(
        "Choose a workflow",
        ("Move large files", "Targeted sends and download recovery"),
        ("Bring text and screenshots over", "Android Share → Paste Stack"),
        ("Show a phone screen", "Screen Share → Freeze & Grab or Live Red Pen"),
    ),
    "windows": readout(
        "Desktop keyboard shortcuts",
        ("D / F", "Dashboard / Files"),
        ("V / T", "Devices / Transfers"),
        ("Shift+S / N", "File search / Shortcut help"),
    ),
    "android": flow(
        "From another Android app",
        ("Select a file", "Open Photos or Files"),
        ("Share → TapSend", "Pick TapSend in Android’s share sheet"),
        ("Computer", "Review the files and send"),
    ),
    "browser": flow(
        "Download in the phone browser",
        ("Open pairing link", "Use the link shown on the PC"),
        ("Find shared files", "Trust the PC’s local CA if needed"),
        ("Download", "Check the browser’s Downloads for the file"),
    ),
    "pairing": diagram(
        "Pair by QR or pass",
        "pairing-diagram", 220,
        "Press P on Windows to show pairing. On Android, open Tools, then Connect a PC. Scan the QR or paste the pairing pass.",
    ),
    "transfers": readout(
        "What the transfer queue shows",
        ("Waiting for phone", "An offer is queued for the selected phone"),
        ("Downloading · 68%", "The phone accepted and is receiving data"),
        ("✓ Saved", "The destination confirmed the file was saved"),
    ),
    "download-recovery": diagram(
        "Resume an interrupted download",
        "resume-diagram", 236,
        "Example showing four chunks stored before a connection drop. Retrying the same file reuses those four chunks and downloads the remaining two.",
    ),
    "messages": readout(
        "Messages and reusable text",
        ("M · Messages", "Send a short message to a phone"),
        ("K · Scratchpad", "Open reusable text for the selected phone"),
        ("P inside Scratchpad", "Share text from the Windows clipboard"),
    ),
    "paste-stack": flow(
        "Send a screenshot to Windows",
        ("Android Share", "Choose TapSend with the screenshot selected"),
        ("Queue or deliver", "Android can hold it until the PC is reachable"),
        ("Windows · Shift+V", "Open Paste Stack and choose the item"),
    ),
    "screen-share": screen_view("live"),
    "freeze-and-grab": screen_view("freeze"),
    "live-red-pen": screen_view("pen"),
    "folder-drop": flow(
        "Send straight to a project folder",
        ("Windows · Shift+R", "Set the open folder as the temporary destination"),
        ("Android · Send here", "Confirm the folder shown on the phone"),
        ("Saved", "Duplicate names are kept with a numbered suffix"),
    ),
    "hold-this-for-me": readout(
        "When a held item returns",
        ("H in Paste Stack", "When you revisit the chosen folder"),
        ("J in Paste Stack", "When the selected phone reconnects"),
        ("B in Paste Stack", "Bring all held items back now"),
    ),
    "file-relay": diagram(
        "A file’s route through File Relay",
        "relay-diagram", 220,
        "The sending PC queues a file. A shared folder holds its encrypted contents until the destination PC returns and saves it.",
    ),
    "releases": readout(
        "Choose the right download",
        ("Windows setup EXE", "Install v1.0.2 with an internet connection"),
        ("Windows portable ZIP", "Extract the complete v1.0.2 bundle"),
        ("Linux installer", "Check dependencies and build the current console app"),
    ),
    "troubleshooting": flow(
        "When a phone can’t connect",
        ("Check the network", "Both devices need to reach each other"),
        ("Check the firewall", "Allow TapSend on the active Windows network"),
        ("Refresh the QR", "Scan a fresh QR after the PC address changes"),
    ),
    "privacy-and-storage": readout(
        "Where your files and settings go",
        ("PC inbox", "Files sent from Android or the phone browser"),
        ("PC private data", "Pairing records, certificates, and private keys"),
        ("Android Downloads/TapSend", "Finished downloads saved by the app"),
    ),
    "cli-and-settings": readout(
        "Common terminal commands",
        ("tabsend devices", "List paired phones"),
        ("tabsend receive", "Open the computer inbox to phone uploads"),
        ("tabsend settings", "Show your saved configuration"),
    ),
    "tui-2": readout(
        "Console file browser controls",
        ("↑ / ↓", "Move the file cursor"),
        ("Space / Enter", "Mark a file / open a directory"),
        ("S / ?", "Start sharing / open help"),
    ),
    "development": flow(
        "How the app builds",
        ("Go", "Build the local transfer server and command-line tools"),
        ("Rust", "Build the terminal interface"),
        ("Gradle · optional", "Build the Android APK with the JDK and SDK"),
    ),
    "releasing": flow(
        "From build to published release",
        ("Package", "Assemble the files for each platform"),
        ("Verify", "Test the packages and generate checksums"),
        ("Publish", "Upload the verified downloads under the intended tag"),
    ),
}
