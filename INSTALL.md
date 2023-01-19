[Read Me](README.md) > Install

## Install
Make sure you removed previous executable:

> sudo rm -f /usr/local/bin/rws #old path
>
> sudo rm -f /usr/bin/rws

[Download precompiled binary](https://github.com/bohdaq/rust-web-server/releases) for you platform from releases page.
There is a mirror for downloads on [Google Drive](https://drive.google.com/drive/folders/13iSR3VxmfFvZgOZ0LddP_EJp7GJ-lQd8?usp=sharing).

You can always [build rws binary](DEVELOPER.md) from source.

## x86_64 Architecture
### Apple macOS
> sudo cp rws /usr/bin
>
> sudo chmod ug+rwx,o+r /usr/bin/rws
#### Homebrew macOS
> brew tap bohdaq/rust-web-server
>
> brew install rws

### Linux
> sudo cp rws /usr/bin
>
> sudo chmod ug+rwx,o+r /usr/bin/rws
#### Debian
> sudo dpkg -i --force-overwrite rws.deb
#### RPM
Replace _VERSION_ with version you downloaded.
> sudo rpm -i --force rws-_VERSION_.rpm
#### Portage ebuild
Open **[Rust Web Server Portage ebuild](https://github.com/bohdaq/rws-gentoo-ebuild)** for details.
#### Pacman package
Open **[Rust Web Server Pacman package](https://github.com/bohdaq/rws-arch-package)** for details.
### Windows
Copy executable to _C:\WINDOWS\system32_ folder.



## ARM Architecture
###  Linux
> sudo cp rws /usr/bin
>
> sudo chmod ug+rwx,o+r /usr/bin/rws
#### Debian
> sudo dpkg -i --force-overwrite rws.deb

###
###

### Testing installation
To check installation execute the following code in the terminal:

> $ rws

You will see similar output:

> Rust Web Server
>
> Version:       YOUR_VERSION
>
> Authors:       Bohdan Tsap <bohdan.tsap@tutanota.com>
>
> Repository:    https://github.com/bohdaq/rust-web-server
>
> Desciption:    rust-web-server (rws) is a static content web-server written in Rust
>
> Rust Version:  RUST_VERSION
> 
> ...
> Hello, rust-web-server is up and running: http://127.0.0.1:7878


Open browser, go to http://127.0.0.1:7878, you'll see default page.

Go back to terminal, press Ctrl + C (or CMD + C) to stop server.