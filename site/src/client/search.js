// Docs search: Pagefind's index (built after prerender into dist/pagefind/),
// loaded on first use. "/" or the sidebar button opens the dialog.
let pagefind;

async function load() {
  // Not bundled by Vite: the file only exists after `pagefind --site dist`.
  const url = "/pagefind/pagefind.js";
  pagefind ??= await import(/* @vite-ignore */ url);
  return pagefind;
}

function escape(text) {
  return text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

const SKELETON = '<div class="search-skeleton"></div>'.repeat(3);

export function initSearch() {
  const dialog = document.querySelector(".search-dialog");
  if (!dialog) return;
  const input = dialog.querySelector("input");
  const results = dialog.querySelector(".search-results");
  let token = 0;
  let timer;

  const open = () => {
    dialog.showModal();
    input.select();
  };
  for (const b of document.querySelectorAll("[data-search-open]")) b.addEventListener("click", open);
  document.addEventListener("keydown", (e) => {
    if (e.key === "/" && !dialog.open && !/^(INPUT|TEXTAREA)$/.test(document.activeElement?.tagName ?? "")) {
      e.preventDefault();
      open();
    }
  });

  input.addEventListener("input", () => {
    clearTimeout(timer);
    const query = input.value.trim();
    const mine = ++token;
    if (!query) {
      results.innerHTML = "";
      return;
    }
    results.innerHTML = SKELETON;
    // Our own debounce: Pagefind's debouncedSearch drops the last query when
    // several arrive while the index is still loading.
    timer = setTimeout(async () => {
      try {
        const search = await (await load()).search(query);
        if (mine !== token) return;
        const items = await Promise.all(search.results.slice(0, 8).map((r) => r.data()));
        if (mine !== token) return;
        results.innerHTML = items.length
          ? `<ul>${items
              .map((d) => `<li><a href="${escape(d.url)}"><b>${escape(d.meta.title ?? d.url)}</b><span>${d.excerpt}</span></a></li>`)
              .join("")}</ul>`
          : `<p class="search-empty">No pages match “${escape(query)}”. Try a setting name like <code>acme</code> or <code>upstream</code>.</p>`;
      } catch {
        if (mine === token) results.innerHTML = '<p class="search-empty">Search is unavailable right now. Use the sidebar or reload the page.</p>';
      }
    }, 150);
  });
}
