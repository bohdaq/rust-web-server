// A line diff for the Config page: what applying the edited text would
// change in the file.

// Past this many cells the table of common lines isn't built: the changed
// middle is shown as removed, then added.
const MAX_CELLS = 4_000_000;

/** The lines of `text`; the newline that ends the file isn't a line. */
function lines(text) {
  const all = text.split("\n");
  if (all.at(-1) === "") all.pop();
  return all;
}

/**
 * `before` → `after`, line by line: `[{ op, text, a, b }]` where `op` is
 * " " (in both), "-" (only in `before`) or "+" (only in `after`), and `a` /
 * `b` are the 1-based line numbers in each (0 where the line isn't).
 */
export function diffLines(before, after) {
  const A = lines(before);
  const B = lines(after);
  let head = 0;
  while (head < A.length && head < B.length && A[head] === B[head]) head++;
  let tailA = A.length;
  let tailB = B.length;
  while (tailA > head && tailB > head && A[tailA - 1] === B[tailB - 1]) {
    tailA--;
    tailB--;
  }
  const out = [];
  for (let i = 0; i < head; i++) out.push({ op: " ", text: A[i], a: i + 1, b: i + 1 });

  const n = tailA - head;
  const m = tailB - head;
  if (n * m > MAX_CELLS) {
    for (let i = 0; i < n; i++) out.push({ op: "-", text: A[head + i], a: head + i + 1, b: 0 });
    for (let j = 0; j < m; j++) out.push({ op: "+", text: B[head + j], a: 0, b: head + j + 1 });
  } else {
    // common[i][j]: the longest run of lines A[head+i..] and B[head+j..] share.
    const width = m + 1;
    const common = new Uint32Array((n + 1) * width);
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        common[i * width + j] = A[head + i] === B[head + j]
          ? common[(i + 1) * width + j + 1] + 1
          : Math.max(common[(i + 1) * width + j], common[i * width + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && A[head + i] === B[head + j]) {
        out.push({ op: " ", text: A[head + i], a: head + i + 1, b: head + j + 1 });
        i++;
        j++;
      } else if (i < n && (j === m || common[(i + 1) * width + j] >= common[i * width + j + 1])) {
        out.push({ op: "-", text: A[head + i], a: head + i + 1, b: 0 });
        i++;
      } else {
        out.push({ op: "+", text: B[head + j], a: 0, b: head + j + 1 });
        j++;
      }
    }
  }
  for (let i = tailA; i < A.length; i++) out.push({ op: " ", text: A[i], a: i + 1, b: i - tailA + tailB + 1 });
  return out;
}

/** How many lines `diff` adds and removes. */
export function counts(diff) {
  let added = 0;
  let removed = 0;
  for (const line of diff) {
    if (line.op === "+") added++;
    else if (line.op === "-") removed++;
  }
  return { added, removed };
}

/**
 * The changed parts of `diff`, each with `context` unchanged lines around
 * it; changes closer than that share a part.
 */
export function hunks(diff, context = 2) {
  const keep = new Uint8Array(diff.length);
  diff.forEach((line, at) => {
    if (line.op === " ") return;
    for (let k = Math.max(0, at - context); k <= Math.min(diff.length - 1, at + context); k++) keep[k] = 1;
  });
  const parts = [];
  let part = null;
  diff.forEach((line, at) => {
    if (!keep[at]) {
      part = null;
      return;
    }
    if (!part) parts.push((part = []));
    part.push(line);
  });
  return parts;
}
