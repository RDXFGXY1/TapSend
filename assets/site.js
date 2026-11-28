"use strict";

document.documentElement.classList.add("has-js");

const menuButton = document.querySelector(".menu-button");
const navigation = document.querySelector(".guide-nav");
menuButton.addEventListener("click", () => {
  const expanded = navigation.classList.toggle("expanded");
  menuButton.setAttribute("aria-expanded", String(expanded));
});

const dialog = document.querySelector(".search-dialog");
const searchButton = document.querySelector(".search-button");
const searchField = dialog.querySelector("input");
const results = dialog.querySelector(".search-results");
const status = dialog.querySelector(".search-status");
const popular = ["quick-start.html", "pairing.html", "screen-share.html", "releases.html"];
let previousFocus;

function normalize(text) {
  return text.toLowerCase().normalize("NFKD").replace(/\p{M}/gu, "");
}

function matchPages(query) {
  if (!query) return popular.map((href) => TAPSEND_SEARCH_INDEX.find((page) => page.href === href)).filter(Boolean);
  const terms = query.split(/\s+/);
  const matches = [];
  for (const page of TAPSEND_SEARCH_INDEX) {
    const title = normalize(page.title);
    const description = normalize(page.description);
    const available = normalize([page.title, page.description, page.text].join(" "));
    if (!terms.every((word) => available.includes(word))) continue;
    const section = page.sections.find((item) => terms.every((word) => normalize(item.title).includes(word)));
    const score = (title.includes(query) ? 20 : 0) + (section ? 10 : 0) +
      terms.reduce((sum, word) => sum + (title.includes(word) ? 5 : description.includes(word) ? 3 : 1), 0);
    matches.push({
      title: section && !title.includes(query) ? `${page.title} / ${section.title}` : page.title,
      description: page.description,
      href: section && !title.includes(query) ? `${page.href}#${section.id}` : page.href,
      score,
    });
  }
  return matches.sort((a, b) => b.score - a.score).slice(0, 10);
}

function renderResults() {
  const query = normalize(searchField.value.trim());
  const pages = matchPages(query);
  results.replaceChildren();
  status.textContent = query
    ? pages.length === 0 ? "No guides found. Try another word." : `${pages.length} ${pages.length === 1 ? "result" : "results"}`
    : "Popular guides";
  for (const page of pages) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    const title = document.createElement("strong");
    const description = document.createElement("span");
    link.href = page.href;
    title.textContent = page.title;
    description.textContent = page.description;
    link.append(title, description);
    item.append(link);
    results.append(item);
  }
}

function openSearch() {
  if (dialog.open) return;
  previousFocus = document.activeElement;
  dialog.showModal();
  searchField.focus();
  renderResults();
}

searchButton.addEventListener("click", openSearch);
searchField.addEventListener("input", renderResults);
dialog.addEventListener("close", () => previousFocus?.focus());
dialog.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});

dialog.addEventListener("keydown", (event) => {
  const links = [...results.querySelectorAll("a")];
  if (!links.length) return;
  const focused = links.indexOf(document.activeElement);
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    if (event.key === "ArrowUp" && focused === 0) {
      searchField.focus();
      return;
    }
    const next = event.key === "ArrowDown" ? Math.min(focused + 1, links.length - 1)
      : focused === -1 ? links.length - 1 : focused - 1;
    links[next].focus();
  } else if (event.key === "Enter" && document.activeElement === searchField) {
    event.preventDefault();
    links[0].click();
  }
});

document.addEventListener("keydown", (event) => {
  if (dialog.open || event.repeat || event.defaultPrevented) return;
  const typing = event.target instanceof HTMLElement &&
    (event.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName));
  const quickSearch = event.key === "/" && !typing && !event.ctrlKey && !event.metaKey && !event.altKey;
  const commandSearch = event.key.toLowerCase() === "k" && (event.ctrlKey || event.metaKey) && !event.altKey;
  if (quickSearch || commandSearch) {
    event.preventDefault();
    openSearch();
  }
});
