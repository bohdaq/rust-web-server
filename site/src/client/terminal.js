// Hero terminal: types each command, prints its output, pauses, loops.
// The prerendered <pre> already shows the finished session, so without JS or
// with reduced motion nothing changes.
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function initTerminal() {
  const pre = document.querySelector("[data-terminal]");
  if (!pre || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const lines = JSON.parse(pre.dataset.terminal);
  const line = (cls, text) => {
    const el = document.createElement("span");
    el.className = `t-line ${cls}`;
    el.textContent = text;
    pre.append(el);
    return el;
  };
  let visible = true;
  new IntersectionObserver(([e]) => (visible = e.isIntersecting)).observe(pre);

  (async function loop() {
    for (;;) {
      pre.replaceChildren();
      for (const l of lines) {
        while (!visible) await sleep(500);
        if (l.cmd) {
          const el = line("", "");
          const prompt = document.createElement("span");
          prompt.className = "t-prompt";
          prompt.textContent = "$ ";
          el.append(prompt);
          const text = document.createTextNode("");
          el.append(text);
          el.classList.add("t-typing");
          for (const ch of l.cmd) {
            text.data += ch;
            await sleep(45 + Math.random() * 40);
          }
          el.classList.remove("t-typing");
          await sleep(350);
        } else {
          line(l.accent ? "t-accent" : "t-out", l.out);
          await sleep(140);
        }
      }
      await sleep(4500);
    }
  })();
}
