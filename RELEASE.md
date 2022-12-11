# Release Info
Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

Minimum rust version is 1.65, as I'm testing on this specific version. However, if needed you may try to build rws on your own using older version with the _--ignore-rust-version_ flag.


## Build

> cargo build --release
>
> cd target/release
>
> ./rws --ip=127.0.0.1 --port=8888 --threads=100


# Release
Build binary on specific platform to prepare release.

For each binary provide sha 256 check sum.

Releases initially being prepared at
[Drive](https://drive.google.com/drive/folders/13iSR3VxmfFvZgOZ0LddP_EJp7GJ-lQd8?usp=share_link) mirror.

There are additional templates for
[Homebrew](https://brew.sh/),
[Debian](https://www.debian.org/) and
[RPM](https://rpm.org/) package systems.


Here is the list of supported architectures:
1. x86 64-bit Apple: **x86_64-apple-darwin**
    1. [Homebrew formula](https://github.com/bohdaq/homebrew-rust-web-server)
2. x86 64-bit Linux: **x86_64-unknown-linux-gnu**
   1.  Debian: **rws-x86_64-create-deb** TODO LINK
   2.  RPM: **rws-x86_64-create-rpm** TODO LINK
3. ARM 64-bit Linux: **aarch64_unknown_linux_gnu**
   1.  Debian: **rws-arm-create-deb** TODO LINK
4. x86 64-bit Windows: **x86_64-pc-windows-msvc**


Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).
