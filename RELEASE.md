[Read Me](README.md) > [Developer](DEVELOPER.md) > Release

# Release Info
Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

Minimum Rust version is **1.75**.

## Build

Plain HTTP/1.1 binary:
> cargo build --release
>
> cd target/release
>
> ./rws --ip=127.0.0.1 --port=8888 --threads=100

HTTPS + HTTP/2 binary:
> cargo build --release --features http2
>
> cd target/release
>
> ./rws --ip=127.0.0.1 --port=443 --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem


# Release
Build binary on specific platform to prepare release.

For each binary provide sha 256 check sum.

Package formats supported: Homebrew, Portage ebuild, Pacman, Debian (.deb), RPM (.rpm).


Here is the list of supported architectures:
1. x86 64-bit Apple: **x86_64-apple-darwin**
1. x86 64-bit Linux: **x86_64-unknown-linux-gnu**
   1.  Debian (.deb)
   1.  RPM (.rpm)
   1.  Portage ebuild
   1.  Pacman package
1. ARM 64-bit Linux: **aarch64_unknown_linux_gnu**
   1.  Debian (.deb)
1. x86 64-bit Windows: **x86_64-pc-windows-msvc**


Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).
