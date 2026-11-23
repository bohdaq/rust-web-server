# kTLS — upstream drafts

Drafts from the kTLS work (`spec/KTLS.md`). Nothing here has been sent.
Owner decides what to file; the user allowed public GitHub issues only for
reproduced bugs.

## 1. Owner's notes

One more item is kept in the owner's own notes.

## 2. Not filed — evaluated

- **Buffered `ServerConnection`/`ClientConnection` have no
  `dangerous_into_kernel_connection` (0.23).** Only the unbuffered API
  returns a `KernelConnection`, so tokio-rustls users can't follow a TLS 1.3
  KeyUpdate (rws closes the connection). PR rustls/rustls#2370 said other
  connection types could follow; rustls `main` has since reworked
  connections (`Connection<Side, T>`, `split`), where the conversion exists
  but is `pub(crate)`. Worth a question on `main`'s design rather than a 0.23
  issue; not filed.
- **`dangerous_extract_secrets` and received data.** rustls/rustls#3230
  made extraction refuse pending *send* data. Received-but-unread plaintext
  (a TLS 1.3 client's first request read with its Finished by tokio-rustls)
  is still silently dropped unless the caller drains it first, as rws and
  `ktls` do. Not checked against `main`'s new API; not filed.
- **Kernel / netdev.** No kernel bug found. The benchmark (no CPU win on 6.8 with
  AES-NI/AVX2, `spec/KTLS.md`) could go to netdev with a newer-kernel
  comparison; the user asked for no kernel mail.

## 3. HITOP

HITOP (the HTTP client app) can't use this directly: kTLS needs a Linux
server-side socket the app controls, and the desktop/mobile clients run on
macOS, Windows, iOS and Android. A HITOP backend on Linux talking HTTPS to
upstreams would get it through rws' proxy (`tls_ktls = true`), measured
there at −4…−11% CPU for 1 MiB bodies and +4…7% for small ones — not worth
it today.
