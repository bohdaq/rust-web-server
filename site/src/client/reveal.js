// The tagline's word-by-word reveal (skill B11). Words only dim once JS has
// added .js to <html>, so the sentence reads in full without JS. Scroll
// reveals for .reveal come from @lit-material/site-kit.
export function initTagline() {
  const words = [...document.querySelectorAll(".tagline .word")];
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    words.forEach((w) => w.classList.add("lit"));
    return;
  }
  // A word lights up when it crosses a line 40% up from the bottom of the
  // viewport, so the sentence fills in reading order as you scroll. Words on
  // one line cross together, so each batch is staggered in reading order
  // rather than flipping at once.
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
