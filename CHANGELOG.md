# Changelog

## 17.113.1 — security patch

Fixes from the 2026-10-25 security review. Upgrade from 17.113.0.

### Security

- **Proxy: interim responses (#48).** A `1xx` from an upstream (`100 Continue`, `103 Early Hints`) was taken as the final response; the real response stayed in the pooled connection and was delivered to the next request on it — possibly another client's. Both forwarders now read past interim responses (at most 16; `101 Switching Protocols` is still final).
- **Static files stay inside the web root (#43).** A request path with `..` segments (`/../../etc/passwd`) was resolved against the working directory and could read any file the process could; such paths now get `404`, and a symlink inside the root that points outside it is not followed. Dot-files and dot-directories (`.git/`, `.env`) are no longer served; set `RWS_CONFIG_SERVE_DOTFILES=true` to serve them again (`.well-known/` is always served).
