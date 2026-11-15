// Docs search in a <dialog> on every page, backed by Pagefind (loaded on
// first use). "/" and Ctrl/Cmd+K open it from anywhere; [data-search-open]
// buttons too. The input is a combobox over a listbox of results: ↑/↓ move,
// Enter opens the highlighted result (the first one by default), Esc
// closes. A page's matching sections are listed under it, each a link to
// its heading. Docs pages search their own version; other pages the latest.
const escape = (text) => String(text).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const SKELETON = '<div class="search-skeleton"></div>'.repeat(3);
const pagefind = () => import(/* @vite-ignore */ "/pagefind/pagefind.js");
// Heading anchors ("#") are ignored at index time; strip any left in excerpts.
const clean = (excerpt) => String(excerpt ?? "").replace(/#\s*(?=<mark>|[A-Z])/g, " ");

export function initSearch({ emptyHint = "", limit = 8, delay = 150 } = {}) {
  const dialog = document.querySelector(".search-dialog");
  const input = dialog?.querySelector("input");
  const results = dialog?.querySelector(".search-results");
  if (!dialog || !input || !results) return;
  let index;
  let token = 0;
  let timer;
  let active = -1;
  const version = () => document.querySelector("[data-docs-version]")?.dataset.docsVersion || dialog.dataset.latestVersion || "";
  const options = () => (version() ? { filters: { version: version() } } : {});
  const options$ = () => [...results.querySelectorAll('[role="option"]')];

  const highlight = (i) => {
    const list = options$();
    if (!list.length) {
      active = -1;
      input.removeAttribute("aria-activedescendant");
      return;
    }
    active = (i + list.length) % list.length;
    list.forEach((el, n) => el.setAttribute("aria-selected", String(n === active)));
    input.setAttribute("aria-activedescendant", list[active].id);
    list[active].scrollIntoView({ block: "nearest" });
  };

  const open = () => {
    if (dialog.open) return;
    dialog.showModal();
    input.select();
  };
  for (const b of document.querySelectorAll("[data-search-open]")) b.addEventListener("click", open);
  document.addEventListener("keydown", (e) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName ?? "") || document.activeElement?.isContentEditable;
    if ((e.key === "/" && !typing) || (e.key.toLowerCase() === "k" && (e.ctrlKey || e.metaKey))) {
      e.preventDefault();
      open();
    }
  });
  // A click on the backdrop closes the dialog.
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog) dialog.close();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      dialog.close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      highlight(active + (e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Enter") {
      // The form is method=dialog: without this Enter only closed it.
      e.preventDefault();
      const target = options$()[Math.max(0, active)];
      if (target) location.assign(target.getAttribute("href"));
    }
  });
  input.addEventListener("input", () => {
    clearTimeout(timer);
    const query = input.value.trim();
    const mine = ++token;
    active = -1;
    input.removeAttribute("aria-activedescendant");
    if (!query) {
      results.innerHTML = "";
      input.setAttribute("aria-expanded", "false");
      return;
    }
    results.innerHTML = SKELETON;
    timer = setTimeout(async () => {
      try {
        index ??= pagefind();
        const search = await (await index).search(query, options());
        if (mine !== token) return;
        const items = await Promise.all(search.results.slice(0, limit).map((r) => r.data()));
        if (mine !== token) return;
        let n = 0;
        const option = (href, inner, cls = "") => `<a role="option" id="search-option-${n++}" aria-selected="false" class="${cls}" href="${escape(href)}">${inner}</a>`;
        results.innerHTML = items.length
          ? `<div role="listbox" aria-label="Results">${items
              .map((d) => {
                const subs = (d.sub_results ?? []).filter((s) => s.url !== d.url && s.title !== d.meta.title).slice(0, 3);
                return `<div class="search-page">${option(d.url, `<b>${escape(d.meta.title ?? d.url)}</b><span>${clean(d.excerpt)}</span>`)}${subs
                  .map((s) => option(s.url, `<b>${escape(s.title)}</b><span>${clean(s.excerpt)}</span>`, "search-sub"))
                  .join("")}</div>`;
              })
              .join("")}</div>`
          : `<p class="search-empty">No pages match “${escape(query)}”.${emptyHint ? ` ${emptyHint}` : ""}</p>`;
        input.setAttribute("aria-expanded", String(items.length > 0));
        if (items.length) highlight(0);
      } catch {
        index = undefined;
        if (mine === token) results.innerHTML = '<p class="search-empty">Search is unavailable right now. Use the navigation or reload the page.</p>';
      }
    }, delay);
  });
  results.addEventListener("pointermove", (e) => {
    const el = e.target.closest('[role="option"]');
    if (el) highlight(options$().indexOf(el));
  });
}
