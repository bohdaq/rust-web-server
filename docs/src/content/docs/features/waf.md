---
title: Web application firewall
description: rws's built-in WAF — SQL injection, XSS, path traversal, command injection, Log4Shell, SSRF and scanner detection with anomaly scoring, detect or block, exclusions and custom rules.
---

Since 18.22.0 rws checks requests itself before they reach your app. No
rule language to learn and no external engine: a table in the config file.

```toml
[middleware.waf]              # every route (and the built-in server); or [route.middleware.waf] per route
# mode     = "block"          # block (default) | detect (log only) | off
# paranoia = 1                # 1–3: higher adds stricter, noisier rules
# threshold = 5               # anomaly score that refuses a request (5 = one critical match)
# inspect_body_bytes = 65536  # how much of a body is checked (0: bodies aren't)
# max_args = 255              # more arguments than this is a match
exclude = ["942100@/api/search", "941*@/cms/"]   # rule[@path prefix] off where it misfires

[[middleware.waf.rule]]       # your own rules
id     = 1001
field  = "path"               # path | query | args | arg:<name> | header:<name> | method | ip | body
prefix = ["/wp-login.php", "/xmlrpc.php"]   # or contains = […], equals = […], cidr = […] for ip
action = "deny"               # deny | allow (skip the WAF) | log | score (+ score = 5)
```

## What it checks

The path, query arguments, form fields, JSON string values, the non-file
fields of multipart forms (and uploaded file names), cookie values, the
`User-Agent`, and every header for Log4Shell. Values are decoded first —
percent-encoding (also double), HTML entities, `\x` / `\u` escapes, SQL
`/* comments */` — so encoded attacks are seen as what they decode to.

| Rules | Detects | Paranoia |
|---|---|---|
| 942100–942160 | SQL injection: boolean logic or a comment after a quote (`' or '1'='1`, `admin'--`), `UNION SELECT`, stacked queries (`; drop table`), `sleep()` / `benchmark()` / `waitfor delay`, `information_schema` / `load_file` / `into outfile`, `or 1=1` | 1 |
| 942200, 942300 | `SELECT … FROM` in a value; `--` / `#` comments | 2, 3 |
| 941100–941140 | XSS: `<script>`, event handlers in tags or after a broken-out attribute (`<img onerror=…>`, `" onmouseover="…`), `javascript:` / `vbscript:` / `data:text/html` URLs, `<iframe>` / `<object>` / `<embed>` / `<base>` / `<meta>`, CSS `expression()` / `srcdoc` | 1 |
| 941300 | any HTML tag in a value | 3 |
| 930100, 930110 | path traversal (`../../`), sensitive files (`/etc/passwd`, `win.ini`, `.git/`, `.env`, `wp-config.php`, `id_rsa`) | 1 |
| 931100 | inclusion wrappers: `php://`, `phar://`, `expect://`, `file://`, `gopher://`, `data://` | 1 |
| 932100, 932110 | command injection: a shell command after `;`, `\|`, `&&`, a backtick or `$(`; `$IFS` evasion | 1 |
| 932200 | shell substitution syntax | 2 |
| 934100 | SSRF to cloud metadata (`169.254.169.254`, `metadata.google.internal`, …) | 1 |
| 944100 | Log4Shell JNDI lookups, obfuscated ones (`${${lower:j}ndi:…}`) included | 1 |
| 913100 | security scanners by User-Agent (sqlmap, nikto, nuclei, wpscan, …) | 1 |
| 920100, 920200 | too many arguments; a value over 8 KiB | 1, 2 |

The ids follow the OWASP Core Rule Set's families so they're familiar; the
rules themselves are rws's own and much smaller than the CRS. Paranoia 1 is
tuned so ordinary text (`O'Reilly`, `salt & pepper; to taste`, `Union
Station`, `x < y and y > z`) passes — the test suite carries an attack and a
benign corpus.

## What happens on a match

Each match adds its score (5 for paranoia-1 rules). At `threshold` the
request is refused with `403` and an RFC 9457 problem+json body naming the
rules; nothing reaches the app. Every match is logged — `[waf] blocked
203.0.113.7 GET /search?q=… score=10 942100@arg:q 942110@arg:q` — and
counted: `rws_waf_requests_total{action="blocked"|"detected"}` and
`rws_waf_rule_matches_total{rule="…"}` on `/metrics`.

Start new sites with `mode = "detect"` for a few days, read the log, add
exclusions for what misfires, then switch to `block`. A custom `allow` rule
(an office network, a trusted health checker) skips the WAF; a `deny` rule
refuses whatever the mode.

## Placement and cost

Per route the WAF runs after the IP filter and rate limit and before auth,
so login pages are covered. A request without matches costs a few
microseconds in a release build. Request bodies on a WAF route are read
whole rather than streamed (only the first `inspect_body_bytes` are
inspected), so give such routes a `max_body_size`; `rws --check` warns when
neither it nor the global `max_body_size_in_bytes` is set. Routes with a WAF (like any
middleware) use the blocking path rather than the non-blocking proxy
forwarder. It's a layer, not a substitute for parameterized queries and
output escaping in the app.

From Rust: `rust_web_server::waf::WafLayer::new(WafConfig { .. })` on any
app with `.wrap(…)`.
