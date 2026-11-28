# TapSend documentation website

The website renders the Markdown in [docs/](../docs/) to HTML. The guide pages and navigation work without JavaScript. Search and the compact navigation use a small local script. Artwork comes from HTML and CSS, and fonts come from the Android app's bundled JetBrains Mono files.

The landing page at `index.html` has a desktop illustration, direct downloads, and five interactive examples. The guide hub is at `docs.html`. Each guide has a matching visual defined in [visuals.py](visuals.py). Pairing, File Relay, and download recovery use SVG diagrams from `site/assets/`. [Open the published website](https://rdxfgxy1.github.io/TapSend/)

## Preview locally

```text
python -m pip install -r site/requirements.txt
python site/build.py
python -m http.server 8090 --bind 127.0.0.1 --directory dist/tapsend-site
```

Open [the local preview](http://127.0.0.1:8090/) or open `dist/tapsend-site/index.html` directly. Re-run the build after editing a guide or a site asset.

## Publish on GitHub Pages

The live site currently publishes from the root of `gh-pages`. Build the site on `tap-send`, then publish the contents of `dist/tapsend-site/` to `gh-pages`, including `.nojekyll` and the `downloads/` directory.

### Custom workflow

1. Push the website source, documentation, and [workflow](../.github/workflows/docs-pages.yml) to GitHub. The workflow must be present on the repository's default branch for manual runs.
2. Open **Settings → Pages** and set **Source** to **GitHub Actions**.
3. In **Actions**, choose **Publish TapSend Docs** and select **Run workflow**. The deploy job reports the resulting Pages URL. [GitHub Pages settings guide](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site)

The workflow uploads only the generated website in `dist/tapsend-site/`. The public Releases page links to GitHub release assets and lists workspace builds by filename. Local APKs and ZIPs stay in the workspace.

## Add a guide

Add a Markdown file under `docs/`, link it from `docs/README.md`, and place its filename stem in the appropriate group in `site/build.py`. Use an H1 for the page title and add a visual for the guide in `site/visuals.py`. The build creates the page, navigation, heading links, and search entry.
