import json
import os
import re
import shutil
from dataclasses import dataclass
from hashlib import sha256
from html import escape
from pathlib import Path
from urllib.parse import quote, urlsplit

from markdown_it import MarkdownIt

from visuals import GUIDE_VISUALS, desktop_preview, screen_view


ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SITE = ROOT / "site"
OUTPUT = ROOT / "dist" / "tapsend-site"
REPO = f"https://github.com/{os.environ.get('GITHUB_REPOSITORY', 'RDXFGXY1/TapSend')}"
SOURCE_BRANCH = os.environ.get("GITHUB_REF_NAME", "tap-send")
STYLE_REVISION = sha256((SITE / "assets/site.css").read_bytes()).hexdigest()[:12]
WINDOWS_SETUP = f"{REPO}/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64-setup.exe"
INDEX_ROW = re.compile(r"^\| \[([^\]]+)\]\(([\w-]+)\.md\) \| (.+?) \|$", re.MULTILINE)
LOCAL_ASSET = re.compile(r"\[([^\]]+)\]\(\.\./dist/[^)]+\)")
MARKDOWN = MarkdownIt("commonmark", {"html": False}).enable("table")
MARKDOWN.renderer.rules["table_open"] = lambda *_: '<div class="table-scroll" role="region" aria-label="Table" tabindex="0"><table>\n'
MARKDOWN.renderer.rules["table_close"] = lambda *_: "</table></div>\n"

GROUPS = {
    "Get started": ("quick-start", "features", "windows", "android", "browser", "pairing"),
    "Workflows": (
        "transfers", "download-recovery", "messages", "paste-stack", "screen-share",
        "freeze-and-grab", "live-red-pen", "folder-drop", "hold-this-for-me", "file-relay",
    ),
    "Reference": (
        "releases", "troubleshooting", "privacy-and-storage", "cli-and-settings",
        "tui-2", "development", "releasing",
    ),
}


@dataclass(frozen=True)
class Guide:
    slug: str
    title: str
    description: str

    @property
    def href(self):
        return f"{self.slug}.html"


def read_guides():
    index = (DOCS / "README.md").read_text(encoding="utf-8-sig")
    guides = {slug: Guide(slug, title, description) for title, slug, description in INDEX_ROW.findall(index)}
    files = {file.stem for file in DOCS.glob("*.md")} - {"README"}
    grouped = [slug for slugs in GROUPS.values() for slug in slugs]
    if files != guides.keys() or files != set(grouped) or len(grouped) != len(files):
        raise ValueError("Every Markdown guide must appear once in docs/README.md and the website navigation.")
    if files != GUIDE_VISUALS.keys():
        raise ValueError("Every guide needs a visual example in site/visuals.py.")
    return guides


def source_url(relative):
    return f"{REPO}/blob/{SOURCE_BRANCH}/{quote(str(relative).replace(chr(92), '/'))}"


def slug_heading(text, used):
    base = re.sub(r"[^\w -]", "", text.casefold()).replace(" ", "-").strip("-") or "section"
    slug = base
    suffix = 1
    while slug in used:
        slug = f"{base}-{suffix}"
        suffix += 1
    used.add(slug)
    return slug


def rewrite_link(guide, value, guides):
    parts = urlsplit(value)
    if parts.scheme or parts.netloc or not parts.path:
        return value
    requested = (DOCS / parts.path).resolve()
    if requested.parent == DOCS and requested.suffix == ".md":
        if requested.stem == "README":
            target = "index.html"
        elif requested.stem in guides:
            target = guides[requested.stem].href
        else:
            raise ValueError(f"{guide.slug}: unknown guide {value}")
    elif requested.is_relative_to(ROOT) and requested.exists():
        if requested.is_dir():
            target = f"{REPO}/tree/{SOURCE_BRANCH}/{quote(requested.relative_to(ROOT).as_posix())}"
        else:
            target = source_url(requested.relative_to(ROOT))
    else:
        raise ValueError(f"{guide.slug}: unknown local link {value}")
    return target + (f"#{parts.fragment}" if parts.fragment else "")


def render_guide(guide, guides):
    source = (DOCS / f"{guide.slug}.md").read_text(encoding="utf-8-sig")
    # Workspace assets are ignored by Git and are not part of the public website.
    source = LOCAL_ASSET.sub(r"`\1`", source)
    tokens = MARKDOWN.parse(source)
    if tokens[0].type != "heading_open" or tokens[0].tag != "h1":
        raise ValueError(f"{guide.slug}: start the document with an H1")
    title = tokens[1].content
    tokens = tokens[3:]
    sections = []
    used = set()
    for index, token in enumerate(tokens):
        if token.type == "heading_open":
            label = tokens[index + 1].content
            anchor = slug_heading(label, used)
            token.attrSet("id", anchor)
            if token.tag in ("h2", "h3"):
                sections.append((token.tag, label, anchor))
        for child in token.children or []:
            if child.type == "link_open":
                href = child.attrGet("href")
                child.attrSet("href", rewrite_link(guide, href, guides))
    content = MARKDOWN.renderer.render(tokens, MARKDOWN.options, {})
    text = " ".join(
        " ".join(child.content for child in (token.children or []) if child.type in ("text", "code_inline"))
        for token in tokens if token.type == "inline"
    )
    return title, content, sections, re.sub(r"\s+", " ", text).strip()


def navigation(current, guides):
    groups = []
    for group, slugs in GROUPS.items():
        links = []
        for slug in slugs:
            guide = guides[slug]
            active = ' aria-current="page"' if current == slug else ""
            links.append(f'<li><a href="{guide.href}"{active}><span>{escape(guide.title)}</span></a></li>')
        groups.append(f'<div class="nav-group"><h2>{group}</h2><ul>{"".join(links)}</ul></div>')
    return f'<nav id="guide-navigation" class="guide-nav" aria-label="Documentation">{"".join(groups)}</nav>'


def layout(title, description, current, content, guides):
    page_title = "TapSend Docs" if current == "home" else f"{title} · TapSend Docs"
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark">
  <meta name="description" content="{escape(description)}">
  <title>{escape(page_title)}</title>
  <link rel="icon" href="data:,">
  <link rel="stylesheet" href="assets/site.css?v={STYLE_REVISION}">
  <script defer src="assets/search-index.js"></script>
  <script defer src="assets/site.js"></script>
</head>
<body>
  <a class="skip" href="#main">Skip to content</a>
  <header class="topbar">
    <div class="top-inner">
      <a class="brand" href="index.html" aria-label="TapSend documentation home"><span class="brand-icon" aria-hidden="true">&gt;_</span><strong>TapSend</strong><span class="brand-section">/ DOCS</span></a>
      <div class="top-actions">
        <button class="menu-button" type="button" aria-controls="guide-navigation" aria-expanded="false">Guides</button>
        <button class="search-button" type="button" aria-label="Search docs" aria-haspopup="dialog"><span class="search-icon" aria-hidden="true"></span><span>Search docs</span><kbd>/</kbd></button>
        <a class="top-release" href="releases.html">Downloads</a>
      </div>
    </div>
  </header>
  <div class="shell">
    {navigation(current, guides)}
    <main id="main" tabindex="-1">{content}</main>
  </div>
  <footer class="footer"><div><a href="index.html">TapSend Docs</a><a href="{REPO}">Source on GitHub</a></div></footer>
  <dialog class="search-dialog" aria-labelledby="search-title">
    <form method="dialog" class="search-header"><h2 id="search-title">Search documentation</h2><button type="submit" aria-label="Close search">Esc</button></form>
    <label class="search-field"><span class="search-icon" aria-hidden="true"></span><span class="sr-only">Search terms</span><input type="search" placeholder="Find a guide or command…" autocomplete="off"></label>
    <div class="search-scroll">
      <p class="search-status" role="status"></p>
      <ol class="search-results"></ol>
    </div>
    <div class="search-footer"><span><kbd>↑</kbd> <kbd>↓</kbd> Browse</span><span><kbd>Enter</kbd> Open</span></div>
  </dialog>
</body>
</html>
"""


def home(guides):
    cards = (
        ("[+]", "quick-start", "Quick start", "Install TapSend, pair a phone, and send your first file."),
        ("[⇄]", "transfers", "File transfers", "Choose a device, queue files, and see when they’re saved."),
        ("[◫]", "screen-share", "Screen Share", "View Android live, grab a frame, or point with Live Red Pen."),
        ("[≡]", "paste-stack", "Paste Stack", "Send screenshots and text to Windows, even while the PC is offline."),
        ("[↓]", "folder-drop", "Folder drops", "Send from Android straight into the folder you’re working in."),
        ("[···]", "file-relay", "File Relay", "Stage an encrypted transfer until the destination PC returns."),
    )
    card_markup = "".join(
        f'<a class="guide-card" href="{guides[slug].href}"><span class="card-mark" aria-hidden="true">{mark}</span>'
        f'<div><h3>{title}</h3><p>{description}</p></div></a>'
        for mark, slug, title, description in cards
    )
    return f"""
      <section class="hero" aria-labelledby="home-title">
        <div class="hero-copy">
          <p class="prompt"><span aria-hidden="true">~/</span> documentation</p>
          <h1 id="home-title">TapSend<span>.</span></h1>
          <p class="hero-description">Move files, text, and live screens across your local network.</p>
          <div class="hero-actions">
            <a class="button primary" href="{WINDOWS_SETUP}" download>↓ Windows EXE</a>
            <a class="button" href="downloads/install.sh" download>↓ Linux installer</a>
          </div>
          <p class="download-note">Windows x64 · Linux x64 and ARM64</p>
          <a class="hero-link" href="releases.html">Portable downloads and install instructions →</a>
        </div>
        {desktop_preview()}
      </section>
      {examples()}
      <section class="guide-overview" aria-labelledby="start-title">
        <div class="section-heading"><h2 id="start-title">Explore the guides</h2><a href="features.html">All features</a></div>
        <div class="guide-grid">{card_markup}</div>
      </section>
      <div class="support-panel"><div><h2>Need help connecting?</h2><p>Find pairing fixes, download recovery, and Screen Share help.</p></div><a class="button" href="troubleshooting.html">Troubleshooting</a></div>
    """


def examples():
    return f"""
      <section class="examples" aria-labelledby="examples-title">
        <div class="section-heading"><h2 id="examples-title">See TapSend in action</h2></div>
        <div class="example-tabs" aria-label="Workflow examples">
          <a href="#example-send">Send files</a>
          <a href="#example-linux">Linux setup</a>
          <a href="#example-stack">Paste Stack</a>
          <a href="#example-screen">Screen Share</a>
          <a href="#example-folder">Folder drop</a>
        </div>
        <article class="example" id="example-send">
          <div class="example-copy">
            <h3>Send to a specific phone</h3>
            <p>Pick the phone, mark the files, and send. The queue updates when your phone accepts, downloads, and confirms the save.</p>
            <div class="example-keys"><span><kbd>V</kbd> Device</span><span><kbd>Space</kbd> Mark</span><span><kbd>S</kbd> Send</span></div>
            <a href="transfers.html">File transfer guide →</a>
          </div>
          <div class="example-terminal">
            <div class="example-bar"><span>⇄ &nbsp; Send queue</span><span>Pixel 8</span></div>
            <ul class="example-list">
              <li><span>session-notes.txt</span><strong>✓ Saved</strong></li>
              <li><span>field-footage.mp4</span><span>68% · Downloading</span></li>
              <li class="example-meter"><progress aria-label="Example download progress" max="100" value="68">68%</progress></li>
              <li><span>reference.zip</span><span>Waiting for phone</span></li>
            </ul>
          </div>
        </article>
        <article class="example" id="example-linux">
          <div class="example-copy">
            <h3>See what Linux setup needs</h3>
            <p>Review installed, missing, and outdated tools before approving any changes. Build the Android APK on x64 Linux to download it on your phone using a local QR.</p>
            <a href="development.html#interactive-linux-installer">Linux installer guide →</a>
          </div>
          <div class="example-terminal">
            <div class="example-bar"><span>&gt;_ &nbsp; Linux setup</span></div>
            <ul class="example-list">
              <li><span>Go 1.26 · requires 1.24+</span><strong>✓ Ready</strong></li>
              <li><span>Rust 1.86 · requires 1.88+</span><span class="example-attention">Upgrade needed</span></li>
              <li><span>Java JDK · optional</span><span class="example-attention">Missing</span></li>
            </ul>
            <div class="example-prompt">Install desktop dependencies? <kbd>y</kbd> <kbd>N</kbd></div>
          </div>
        </article>
        <article class="example" id="example-stack">
          <div class="example-copy">
            <h3>Share to Paste Stack</h3>
            <p>On Android, share a screenshot or text to TapSend. Items can queue while the PC is offline. Open the stack on Windows to get them when you reconnect.</p>
            <div class="example-keys"><span><kbd>Shift</kbd> + <kbd>V</kbd> Open stack</span></div>
            <a href="paste-stack.html">Paste Stack guide →</a>
          </div>
          <div class="example-terminal">
            <div class="example-bar"><span>≡ &nbsp; Paste Stack</span><span>Android → PC</span></div>
            <ul class="example-list">
              <li><span>Screenshot_1420.png</span><strong>Image</strong></li>
              <li><span>Meet at the north entrance.</span><strong>Text</strong></li>
            </ul>
            <div class="example-prompt">Two items received</div>
          </div>
        </article>
        <article class="example" id="example-screen">
          <div class="example-copy">
            <h3>View a phone screen on Windows</h3>
            <p>Open the viewer and start Screen Share on Android. The phone chooses what to share. Freeze a frame to save an image or read text.</p>
            <div class="example-keys"><span><kbd>L</kbd> Open viewer</span><span><kbd>Space</kbd> Freeze</span></div>
            <a href="screen-share.html">Screen Share guide →</a>
          </div>
          {screen_view("live")}
        </article>
        <article class="example" id="example-folder">
          <div class="example-copy">
            <h3>Send straight into a folder</h3>
            <p>Choose a paired phone and open the destination folder. The phone confirms where the file will go before sending.</p>
            <div class="example-keys"><span><kbd>Shift</kbd> + <kbd>R</kbd> Set folder drop</span></div>
            <a href="folder-drop.html">Folder drop guide →</a>
          </div>
          {GUIDE_VISUALS["folder-drop"]}
        </article>
      </section>
    """


def article(guide, title, content, sections):
    group = next(name for name, slugs in GROUPS.items() if guide.slug in slugs)
    items = "".join(
        f'<li class="{level}"><a href="#{anchor}">{escape(label)}</a></li>'
        for level, label, anchor in sections
    )
    toc = f'<aside class="contents"><nav aria-label="On this page"><h2>On this page</h2><ul>{items}</ul></nav></aside>' if items else ""
    return f"""
      <div class="article-layout">
        <article class="article">
          <nav class="breadcrumbs" aria-label="Breadcrumb"><a href="index.html">Docs</a> <span aria-hidden="true">/</span> <span>{group}</span></nav>
          <h1>{escape(title)}</h1>
          <p class="article-intro">{escape(guide.description)}</p>
          {GUIDE_VISUALS[guide.slug]}
          <div class="prose">{content}</div>
          <div class="article-footer"><a href="{source_url(f'docs/{guide.slug}.md')}">View Markdown on GitHub</a></div>
        </article>
        {toc}
      </div>
    """


def main():
    guides = read_guides()
    pages = {"index.html": layout("TapSend Docs", "Guides, features, and releases for TapSend.", "home", home(guides), guides)}
    search = []
    for slug, guide in guides.items():
        title, content, sections, text = render_guide(guide, guides)
        pages[guide.href] = layout(title, guide.description, slug, article(guide, title, content, sections), guides)
        search.append({
            "title": title, "description": guide.description, "href": guide.href,
            "sections": [{"title": label, "id": anchor} for _, label, anchor in sections],
            "text": text,
        })
    # Regenerate only files owned by this build. Preserve unrelated outputs under dist/.
    assets = OUTPUT / "assets"
    assets.mkdir(parents=True, exist_ok=True)
    downloads = OUTPUT / "downloads"
    downloads.mkdir(exist_ok=True)
    shutil.copyfile(ROOT / "install.sh", downloads / "install.sh")
    for name, page in pages.items():
        (OUTPUT / name).write_text(page, encoding="utf-8")
    for asset in (SITE / "assets").iterdir():
        shutil.copyfile(asset, assets / asset.name)
    for name in ("jetbrains_mono_regular.ttf", "jetbrains_mono_bold.ttf"):
        shutil.copyfile(ROOT / "android/app/src/main/res/font" / name, assets / name)
    shutil.copyfile(ROOT / "android/JetBrainsMono-OFL.txt", assets / "JetBrainsMono-OFL.txt")
    search_data = json.dumps(search, separators=(",", ":"))
    (assets / "search-index.js").write_text(f"const TAPSEND_SEARCH_INDEX = {search_data};\n", encoding="utf-8")
    (OUTPUT / ".nojekyll").touch()
    print(f"Built {len(pages)} pages in {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
