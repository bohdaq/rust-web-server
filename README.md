# rust-web-server

rust-web-server (**rws**) is a static content web-server written in Rust.

## Features
1. [Cross-Origin Resource Sharing (CORS)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
1. [HTTP Range Requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests)
1. [HTTP Client Hints](https://developer.mozilla.org/en-US/docs/Web/HTTP/Client_hints)
1. [X-Content-Type-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Content-Type-Options)
1. [X-Frame-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Frame-Options). Site is not allowed to be embedded into iframe on other domains. 
1. [Symlinks](https://en.wikipedia.org/wiki/Symbolic_link). You can have symlinks in your folder and they will be resolved correctly.
1. [Caching](https://developer.mozilla.org/en-US/docs/Web/HTTP/Caching#dealing_with_outdated_implementations) done right. It means no caching and therefore no outdated uncontrollable resources.
1. New feature - resolving .html files without .html in path. It means if you try to open /some-html-file it will open file some-html-file.html and won't show 404 not found error
1. No third party dependencies.

## Documentation
Open [documentation](src/README.md) for details.

## Download
Download binary from [Google Drive](https://drive.google.com/drive/folders/13iSR3VxmfFvZgOZ0LddP_EJp7GJ-lQd8?usp=share_link).


## Installation
Open [INSTALL](INSTALL.md) for details.

## Development
Open [DEVELOPER](DEVELOPER.md) for details.

## Configuration
Open [CONFIGURE](CONFIGURE.md) for details.

## Frequently Asked Questions
Open [FAQ](FAQ.md) for details.

## Community
Use GitHub [discussions](https://github.com/bohdaq/rust-web-server/discussions), [issues](https://github.com/bohdaq/rust-web-server/issues) and [pull requests](https://github.com/bohdaq/rust-web-server/pulls).

There is Rust Web Server [Discord](https://discord.gg/zaErjtr5Dm) where you can ask questions and share ideas.

Follow the [Rust code of conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Donations
[PayPal](https://www.paypal.com/donate/?hosted_button_id=7J69SYZWSP6HJ) page to send donations, so I can buy some whole plant food, or open vinyl pressing facility or spend my time snowboarding, whatever.

## Links
1. [Rust TLS Server](https://github.com/bohdaq/rust-tls-server)
1. [http-to-https-letsencrypt](https://github.com/bohdaq/http-to-https-letsencrypt)
1. [Rust Web Framework](https://github.com/bohdaq/rust-web-framework/)
1. [crypto-ext](https://github.com/bohdaq/crypto-ext/)
1. [file-ext](https://github.com/bohdaq/file-ext/)
