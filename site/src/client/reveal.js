// Scroll reveals (skill B7) and the tagline's word-by-word reveal (B11).
// Both only hide content once JS has added .js to <html>, so the page reads
// in full without JS; each element reveals once.
export function initReveal() {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const once = (els, options, cls = "in") => {
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          e.target.classList.add(cls);
          io.unobserve(e.target);
        }
      }
    }, options);
    els.forEach((el) => io.observe(el));
  };
  once(document.querySelectorAll(".reveal"), { rootMargin: "0px 0px -10% 0px" });
  // A word lights up when it crosses a line 40% up from the bottom of the
  // viewport, so the sentence fills in reading order as you scroll.
  // Words on one line cross together, so each batch is staggered in reading
  // order rather than flipping at once.
  const words = [...document.querySelectorAll(".tagline .word")];
  if (reduce) {
    words.forEach((w) => w.classList.add("lit"));
    return;
  }
  const io = new IntersectionObserver((entries) => {
    entries
      .filter((e) => e.isIntersecting)
      .sort((a, b) => words.indexOf(a.target) - words.indexOf(b.target))
      .forEach((e, i) => {
        e.target.style.transitionDelay = `${i * 90}ms`;
        e.target.classList.add("lit");
        io.unobserve(e.target);
      });
  }, { rootMargin: "0px 0px -40% 0px" });
  words.forEach((w) => io.observe(w));
}
