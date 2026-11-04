//! A unified diff of two texts, by lines (longest common subsequence), with
//! three lines of context — what a conflict shows.

/// Above this many line pairs the diff is one hunk replacing everything
/// (the table would take too much memory); generated files are far
/// smaller.
const MAX_CELLS: usize = 4_000_000;

const CONTEXT: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Same,
    Del,
    Add,
}

/// `old` → `new` as a unified diff with headers `--- a/<path>` and
/// `+++ b/<path>`; empty when they are equal. `old: None` is a new file.
pub fn unified(path: &str, old: Option<&str>, new: &str) -> String {
    let old_text = old.unwrap_or("");
    if old.is_some() && old_text == new {
        return String::new();
    }
    let a: Vec<&str> = old_text.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let ops = edit_script(&a, &b);
    let mut out = String::new();
    out.push_str(&match old {
        Some(_) => format!("--- a/{path}\n+++ b/{path}\n"),
        None => format!("--- /dev/null\n+++ b/{path}\n"),
    });
    // Group the script into hunks: changes plus CONTEXT lines around them.
    let changed: Vec<usize> = ops.iter().enumerate().filter(|(_, (op, _, _))| *op != Op::Same).map(|(i, _)| i).collect();
    let mut i = 0;
    while i < changed.len() {
        let first = changed[i];
        let mut last = first;
        while i + 1 < changed.len() && changed[i + 1] <= last + 2 * CONTEXT + 1 {
            i += 1;
            last = changed[i];
        }
        i += 1;
        let start = first.saturating_sub(CONTEXT);
        let end = (last + CONTEXT + 1).min(ops.len());
        let (a_start, b_start) = (ops[start].1, ops[start].2);
        let a_len = ops[start..end].iter().filter(|(op, _, _)| *op != Op::Add).count();
        let b_len = ops[start..end].iter().filter(|(op, _, _)| *op != Op::Del).count();
        let a_from = if a_len == 0 { a_start } else { a_start + 1 };
        let b_from = if b_len == 0 { b_start } else { b_start + 1 };
        out.push_str(&format!("@@ -{a_from},{a_len} +{b_from},{b_len} @@\n"));
        for (op, ai, bi) in &ops[start..end] {
            match op {
                Op::Same => out.push_str(&format!(" {}\n", a[*ai])),
                Op::Del => out.push_str(&format!("-{}\n", a[*ai])),
                Op::Add => out.push_str(&format!("+{}\n", b[*bi])),
            }
        }
    }
    out
}

/// The edit script: each entry is the operation and the indexes into `a`
/// and `b` it is at.
fn edit_script(a: &[&str], b: &[&str]) -> Vec<(Op, usize, usize)> {
    // Common prefix and suffix first: most edits touch a few lines.
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..].iter().rev().zip(b[prefix..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut ops: Vec<(Op, usize, usize)> = (0..prefix).map(|i| (Op::Same, i, i)).collect();
    let (n, m) = (am.len(), bm.len());
    if n.saturating_mul(m) > MAX_CELLS {
        ops.extend((0..n).map(|i| (Op::Del, prefix + i, prefix)));
        ops.extend((0..m).map(|j| (Op::Add, prefix + n, prefix + j)));
    } else {
        // lcs[i][j]: the LCS length of am[i..] and bm[j..].
        let width = m + 1;
        let mut lcs = vec![0u32; (n + 1) * width];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i * width + j] =
                    if am[i] == bm[j] { lcs[(i + 1) * width + j + 1] + 1 } else { lcs[(i + 1) * width + j].max(lcs[i * width + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                ops.push((Op::Same, prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1]) {
                ops.push((Op::Del, prefix + i, prefix + j));
                i += 1;
            } else {
                ops.push((Op::Add, prefix + i, prefix + j));
                j += 1;
            }
        }
    }
    let (a_end, b_end) = (a.len() - suffix, b.len() - suffix);
    ops.extend((0..suffix).map(|k| (Op::Same, a_end + k, b_end + k)));
    ops
}
