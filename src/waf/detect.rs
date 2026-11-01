//! The built-in detections. Each inspects one normalized value
//! ([`super::decode::normalize`]) and reports rule ids. Ids follow the OWASP
//! Core Rule Set's families so they're familiar (942 SQL injection, 941
//! XSS, 930 traversal, 931 remote inclusion, 932 command injection, 934
//! SSRF, 944 Java, 913 scanners, 920 protocol), but the rules are rws's own.

/// One built-in rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    pub id: u32,
    /// Anomaly score added when it matches (5 critical, 3 warning, 2 notice).
    pub score: u32,
    /// The lowest paranoia level it runs at.
    pub paranoia: u8,
    pub message: &'static str,
}

pub const RULES: &[Rule] = &[
    Rule { id: 942100, score: 5, paranoia: 1, message: "SQL injection: boolean logic after a quote" },
    Rule { id: 942110, score: 5, paranoia: 1, message: "SQL injection: comment after a quote" },
    Rule { id: 942120, score: 5, paranoia: 1, message: "SQL injection: UNION SELECT" },
    Rule { id: 942130, score: 5, paranoia: 1, message: "SQL injection: stacked query" },
    Rule { id: 942140, score: 5, paranoia: 1, message: "SQL injection: time-based probe" },
    Rule { id: 942150, score: 5, paranoia: 1, message: "SQL injection: schema or file access" },
    Rule { id: 942160, score: 5, paranoia: 1, message: "SQL injection: numeric tautology" },
    Rule { id: 942200, score: 3, paranoia: 2, message: "SQL injection: SELECT … FROM" },
    Rule { id: 942300, score: 2, paranoia: 3, message: "SQL comment sequence" },
    Rule { id: 941100, score: 5, paranoia: 1, message: "XSS: script tag" },
    Rule { id: 941110, score: 5, paranoia: 1, message: "XSS: event handler attribute" },
    Rule { id: 941120, score: 5, paranoia: 1, message: "XSS: javascript:/vbscript:/data: URL" },
    Rule { id: 941130, score: 5, paranoia: 1, message: "XSS: dangerous tag" },
    Rule { id: 941140, score: 5, paranoia: 1, message: "XSS: CSS or srcdoc injection" },
    Rule { id: 941300, score: 2, paranoia: 3, message: "HTML tag in a value" },
    Rule { id: 930100, score: 5, paranoia: 1, message: "Path traversal" },
    Rule { id: 930110, score: 5, paranoia: 1, message: "Access to a sensitive file" },
    Rule { id: 931100, score: 5, paranoia: 1, message: "Remote or local file inclusion through a URL wrapper" },
    Rule { id: 932100, score: 5, paranoia: 1, message: "Command injection: shell command after a separator" },
    Rule { id: 932110, score: 5, paranoia: 1, message: "Command injection: $IFS evasion" },
    Rule { id: 932200, score: 3, paranoia: 2, message: "Shell substitution syntax" },
    Rule { id: 934100, score: 5, paranoia: 1, message: "SSRF: cloud metadata address" },
    Rule { id: 944100, score: 5, paranoia: 1, message: "Java: JNDI lookup (Log4Shell)" },
    Rule { id: 913100, score: 5, paranoia: 1, message: "Security scanner" },
    Rule { id: 920100, score: 5, paranoia: 1, message: "Too many arguments" },
    Rule { id: 920200, score: 3, paranoia: 2, message: "Argument value too long" },
];

pub fn rule(id: u32) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

// ── SQL tokens ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Quote,
    Word(String),
    Num(String),
    Op(String),
    Comment,
    Paren,
    Semi,
    Other,
}

/// A rough SQL tokenizer: enough to tell `O'Reilly` from `' or 1=1 --`.
fn tokens(s: &str) -> Vec<Tok> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() && out.len() < 4096 {
        let c = b[i];
        if c == ' ' {
            i += 1;
        } else if c == '\'' || c == '"' || c == '`' {
            out.push(Tok::Quote);
            i += 1;
        } else if c.is_ascii_alphabetic() || c == '_' || c == '@' || c == '$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_' || b[i] == '@' || b[i] == '$' || b[i] == '.') {
                i += 1;
            }
            out.push(Tok::Word(b[start..i].iter().collect()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '.') {
                i += 1;
            }
            out.push(Tok::Num(b[start..i].iter().collect()));
        } else if c == '-' && b.get(i + 1) == Some(&'-') || c == '#' {
            out.push(Tok::Comment);
            i += if c == '#' { 1 } else { 2 };
        } else if c == '(' || c == ')' {
            out.push(Tok::Paren);
            i += 1;
        } else if c == ';' {
            out.push(Tok::Semi);
            i += 1;
        } else if "=<>!|&".contains(c) {
            let start = i;
            while i < b.len() && "=<>!|&".contains(b[i]) && i - start < 3 {
                i += 1;
            }
            out.push(Tok::Op(b[start..i].iter().collect()));
        } else {
            out.push(Tok::Other);
            i += 1;
        }
    }
    out
}

fn word(t: Option<&Tok>, words: &[&str]) -> bool {
    matches!(t, Some(Tok::Word(w)) if words.contains(&w.as_str()))
}

fn comparison(t: Option<&Tok>) -> bool {
    matches!(t, Some(Tok::Op(o)) if ["=", "==", "<", ">", "<=", ">=", "<>", "!=", "<=>"].contains(&o.as_str())) || word(t, &["like", "rlike", "regexp", "is", "in", "between", "sounds"])
}

fn logic(t: Option<&Tok>) -> bool {
    word(t, &["or", "and", "xor", "div"]) || matches!(t, Some(Tok::Op(o)) if o == "||" || o == "&&")
}

fn sqli(v: &str, hits: &mut Vec<u32>) {
    if !v.bytes().any(|b| b"'\"`;=()-#".contains(&b) || b == b' ') {
        return;
    }
    let t = tokens(v);
    let at = |i: usize| t.get(i);
    for i in 0..t.len() {
        // 942100: ' or 1=1 / ") or "a"="a / ' || 'x' like 'x
        if t[i] == Tok::Quote {
            let mut j = i + 1;
            while at(j) == Some(&Tok::Paren) {
                j += 1;
            }
            if logic(at(j)) {
                let window = (j + 1..(j + 6).min(t.len())).any(|k| comparison(at(k)) || word(at(k), &["true", "false", "null", "not"]));
                if window || matches!(at(j + 1), Some(Tok::Num(_))) && at(j + 2).is_none() {
                    hits.push(942100);
                }
            }
            // 942110: admin'-- / x' # / ')--
            if matches!(at(j), Some(Tok::Comment)) || (at(j) == Some(&Tok::Semi) && matches!(at(j + 1), Some(Tok::Comment))) {
                hits.push(942110);
            }
        }
        // 942120: union [all|distinct] select
        if word(at(i), &["union"]) {
            let mut j = i + 1;
            while at(j) == Some(&Tok::Paren) {
                j += 1;
            }
            if word(at(j), &["all", "distinct"]) {
                j += 1;
            }
            while at(j) == Some(&Tok::Paren) {
                j += 1;
            }
            if word(at(j), &["select"]) {
                hits.push(942120);
            }
        }
        // 942130: ; drop table / ; select …
        if t[i] == Tok::Semi && word(at(i + 1), &["select", "insert", "update", "delete", "drop", "create", "alter", "truncate", "exec", "execute", "declare", "shutdown", "grant", "revoke", "merge", "call"]) {
            let next = at(i + 2);
            if next.is_some() && !matches!(next, Some(Tok::Other)) {
                hits.push(942130);
            }
        }
        // 942140: sleep(5), benchmark(…), pg_sleep(…), waitfor delay
        if word(at(i), &["sleep", "benchmark", "pg_sleep", "dbms_lock.sleep", "dbms_pipe.receive_message"]) && at(i + 1) == Some(&Tok::Paren) && matches!(at(i + 2), Some(Tok::Num(_)) | Some(Tok::Word(_)) | Some(Tok::Quote)) {
            hits.push(942140);
        }
        if word(at(i), &["waitfor"]) && word(at(i + 1), &["delay", "time"]) {
            hits.push(942140);
        }
        // 942160: or 1=1 / and 2=2
        if logic(at(i)) {
            if let (Some(Tok::Num(a)), Some(Tok::Op(o)), Some(Tok::Num(b))) = (at(i + 1), at(i + 2), at(i + 3)) {
                if a == b && (o == "=" || o == "==" || o == "<=>" || o == ">=" || o == "<=") {
                    hits.push(942160);
                }
            }
        }
        // 942200 (PL2): select … from
        if word(at(i), &["select"]) && (i + 1..(i + 12).min(t.len())).any(|k| word(at(k), &["from"])) {
            hits.push(942200);
        }
    }
    // 942150: schema and file probes
    if ["information_schema", "load_file(", "into outfile", "into dumpfile", "@@version", "xp_cmdshell", "sysobjects", "pg_shadow", "mysql.user", "sqlite_master"].iter().any(|p| v.contains(p)) {
        hits.push(942150);
    }
    if v.contains("--") || v.contains(" #") {
        hits.push(942300);
    }
}

// ── XSS ──────────────────────────────────────────────────────────────────────

/// `v` with spaces and controls removed (`java script:` → `javascript:`).
fn squeezed(v: &str) -> String {
    v.chars().filter(|c| !c.is_whitespace()).collect()
}

fn xss(v: &str, hits: &mut Vec<u32>) {
    if !v.bytes().any(|b| b == b'<' || b == b':' || b == b'=' || b == b'(') {
        return;
    }
    if v.contains("<script") || v.contains("</script") {
        hits.push(941100);
    }
    // 941110: <img src=x onerror=…> / " onmouseover="…
    let bytes = v.as_bytes();
    let mut i = 0;
    while let Some(off) = v[i..].find("on") {
        let at = i + off;
        let before = if at == 0 { b' ' } else { bytes[at - 1] };
        let name_end = at + 2 + v[at + 2..].bytes().take_while(|b| b.is_ascii_alphabetic()).count();
        let name_len = name_end - at - 2;
        let after = v[name_end..].trim_start();
        if name_len >= 3 && after.starts_with('=') && matches!(before, b' ' | b'/' | b'"' | b'\'' | b'`' | b';') {
            // Inside a tag, or right after breaking out of an attribute.
            let tag = v[..at].rfind('<').is_some_and(|lt| !v[lt..at].contains('>'));
            let breakout = v[..at].trim_end().ends_with(['"', '\'', '`']);
            if tag || breakout {
                hits.push(941110);
                break;
            }
        }
        i = at + 2;
        if i >= v.len() {
            break;
        }
    }
    let tight = squeezed(v);
    let url_start = |s: &str| ["javascript:", "vbscript:", "livescript:", "data:text/html", "data:image/svg+xml"].iter().any(|p| s.starts_with(p));
    if url_start(tight.trim_start_matches(['"', '\'', '`'])) || ["=javascript:", "=\"javascript:", "='javascript:", "=vbscript:", "(javascript:", "url(javascript:"].iter().any(|p| tight.contains(p)) {
        hits.push(941120);
    }
    if ["<iframe", "<object", "<embed", "<applet", "<base ", "<base/", "<meta ", "<frameset", "<svg/on", "<math", "<link rel=import", "<form action"].iter().any(|p| v.contains(p)) || (v.contains("<svg") && v.contains("onload")) {
        hits.push(941130);
    }
    if tight.contains("expression(") || tight.contains("-moz-binding") || tight.contains("srcdoc=") || tight.contains("@import") && tight.contains("url(") {
        hits.push(941140);
    }
    if v.find('<').is_some_and(|lt| v[lt + 1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!')) {
        hits.push(941300);
    }
}

// ── Files, inclusion, commands, SSRF, Java ───────────────────────────────────

fn files(v: &str, hits: &mut Vec<u32>) {
    let traversals = v.matches("../").count() + v.matches("..\\").count() + v.matches("..;/").count();
    // One `../` is a relative link; two climb out of where it points.
    if traversals >= 2 {
        hits.push(930100);
    }
    const SENSITIVE: [&str; 17] = [
        "/etc/passwd", "/etc/shadow", "/etc/group", "/proc/self/", "/proc/1/", "c:\\windows\\", "c:/windows/", "boot.ini", "win.ini", "/web.config", "wp-config.php",
        "/.htaccess", "/.htpasswd", "/.git/", "/.ssh/", "id_rsa", "/.aws/credentials",
    ];
    if SENSITIVE.iter().any(|p| v.contains(p)) || v == ".env" || v.ends_with("/.env") || v.contains("../.env") {
        hits.push(930110);
    }
    if ["php://", "phar://", "expect://", "file://", "gopher://", "dict://", "zip://", "jar:file", "data://", "glob://", "ogg://", "ssh2."].iter().any(|p| v.contains(p)) {
        hits.push(931100);
    }
}

const COMMANDS: [&str; 46] = [
    "cat", "ls", "id", "whoami", "uname", "pwd", "wget", "curl", "nc", "ncat", "netcat", "bash", "sh", "zsh", "ksh", "dash", "python", "python3", "perl", "ruby", "php", "ping", "nslookup",
    "dig", "rm", "chmod", "chown", "telnet", "ssh", "scp", "powershell", "cmd", "certutil", "base64", "sudo", "kill", "ifconfig", "ps", "printenv", "systemctl", "tftp", "head", "tail",
    "more", "busybox", "xterm",
];

fn commands(v: &str, hits: &mut Vec<u32>) {
    if !v.bytes().any(|b| b";|&`$\n".contains(&b)) {
        return;
    }
    for sep in [";", "|", "&&", "`", "$(", "\n"] {
        let mut from = 0;
        while let Some(off) = v[from..].find(sep) {
            let after = v[from + off + sep.len()..].trim_start();
            let w: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '/' || *c == '.').collect();
            let name = w.rsplit('/').next().unwrap_or("");
            let rest = &after[w.len()..];
            if COMMANDS.contains(&name) && (rest.is_empty() || rest.starts_with([' ', ';', '|', '&', '`', ')', '<', '>', '$'])) {
                hits.push(932100);
                return;
            }
            from += off + sep.len();
        }
    }
    if v.contains("${ifs}") || v.contains("$ifs") {
        hits.push(932110);
    }
    if v.contains("$(") || v.matches('`').count() >= 2 {
        hits.push(932200);
    }
}

fn ssrf(v: &str, hits: &mut Vec<u32>) {
    if ["169.254.169.254", "metadata.google.internal", "100.100.100.200", "fd00:ec2::254", "instance-data.ec2", "metadata.azure.com", "169.254.170.2"].iter().any(|p| v.contains(p)) {
        hits.push(934100);
    }
}

/// Resolve Log4j's lookup obfuscations (`${lower:j}`, `${::-n}`,
/// `${env:X:-d}`) and look for a JNDI lookup.
pub fn log4shell(v: &str) -> bool {
    if !v.contains("${") {
        return false;
    }
    let mut s = v.to_string();
    for _ in 0..16 {
        let Some(start) = s.rfind("${") else { break };
        let Some(len) = s[start..].find('}') else { break };
        let inner = &s[start + 2..start + len];
        let resolved = if let Some((_, default)) = inner.split_once(":-") {
            default.to_string()
        } else if let Some(rest) = inner.strip_prefix("lower:").or_else(|| inner.strip_prefix("upper:")) {
            rest.to_string()
        } else if inner.starts_with("jndi:") {
            return true;
        } else {
            format!("\u{0}{inner}\u{0}")
        };
        s.replace_range(start..start + len + 1, &resolved);
    }
    s.contains("jndi:ldap") || s.contains("jndi:rmi") || s.contains("jndi:dns") || s.contains("jndi:iiop") || s.contains("jndi:nis") || s.contains("jndi:corba") || s.contains("jndi:http")
}

/// The built-in rules matching one normalized value.
pub fn value(v: &str, hits: &mut Vec<u32>) {
    if v.is_empty() {
        return;
    }
    sqli(v, hits);
    xss(v, hits);
    files(v, hits);
    commands(v, hits);
    ssrf(v, hits);
    if log4shell(v) {
        hits.push(944100);
    }
}

/// Scanner signatures in a (normalized) User-Agent.
pub fn scanner(user_agent: &str) -> bool {
    ["sqlmap", "nikto", "nmap scripting engine", "masscan", "acunetix", "nessus", "openvas", "wpscan", "dirbuster", "gobuster", "fuzz faster u fool", "nuclei", "zgrab", "havij", "w3af", "netsparker", "arachni", "appscan", "jaeles", "zmeu", "morfeus", "paros", "webinspect", "commix"]
        .iter()
        .any(|p| user_agent.contains(p))
}
