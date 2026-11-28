import json
import os
import re
import shutil
from dataclasses import dataclass
from html import escape
from pathlib import Path
from urllib.parse import quote, urlsplit

from markdown_it import MarkdownIt


ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SITE = ROOT / "site"
OUTPUT = ROOT / "dist" / "tapsend-site"
REPO = f"https://github.com/{os.environ.get('GITHUB_REPOSITORY', 'RDXFGXY1/TapSend')}"
SOURCE_BRANCH = os.environ.get("GITHUB_REF_NAME", "tap-send")
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
  <link rel="stylesheet" href="assets/site.css">
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
        <a class="top-release" href="releases.html">Releases</a>
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
          <div class="hero-actions"><a class="button primary" href="quick-start.html">Get started</a><a class="button" href="releases.html">View releases</a></div>
        </div>
        <figure class="terminal-trace">
          <div class="trace-title"><span class="trace-icon" aria-hidden="true">◈</span><figcaption>Example transfer</figcaption><span class="trace-index" aria-hidden="true">[ LAN ]</span></div>
          <div class="trace-content">
            <div class="devices"><div><span class="display-icon" aria-hidden="true"></span>Windows PC</div><span class="connection" aria-hidden="true"><b>⇄</b></span><div><span class="phone-icon" aria-hidden="true"></span>Android</div></div>
            <div class="trace-file"><span>session-notes.txt</span><strong><span aria-hidden="true">✓</span> Saved</strong></div>
          </div>
          <div class="trace-footer"><kbd>V</kbd> Choose device <kbd>S</kbd> Send</div>
        </figure>
      </section>
      <section class="guide-overview" aria-labelledby="start-title">
        <div class="section-heading"><h2 id="start-title">Explore the guides</h2><a href="features.html">All features</a></div>
        <div class="guide-grid">{card_markup}</div>
      </section>
      <div class="support-panel"><div><h2>Need help connecting?</h2><p>Find pairing fixes, download recovery, and Screen Share help.</p></div><a class="button" href="troubleshooting.html">Troubleshooting</a></div>
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
