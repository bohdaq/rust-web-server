---
name: Bug report
about: Something doesn't work as documented
title: ''
labels: bug
assignees: ''

---

**What happened**
What you did and what went wrong. Include the error or log output.

**What you expected**

**How to reproduce**
The smallest `rws.config.toml` and request (`curl …`) that shows the problem.
Run `rws --check` on the config first — it catches most configuration mistakes.

```toml
```

```bash
curl -v …
```

**Environment**
- rws version (`rws --version`):
- How you installed it (release binary / Homebrew / Docker / `cargo install`, with features):
- OS:

Security issues: please report privately instead — see SECURITY.md.
