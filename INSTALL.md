[Read Me](README.md) > Install

## Install
Make sure you removed previous executable:

> sudo rm -f /usr/local/bin/rws #on macOS
>
> sudo rm -f /usr/bin/rws #on Linux

Build the binary from source — see [DEVELOPER](DEVELOPER.md) for instructions.

## x86_64 Architecture
### Apple macOS
> sudo cp rws /usr/local/bin
>
> sudo chmod 777 /usr/local/bin/rws

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
Build from source and install manually.
#### Pacman package
Build from source and install manually.
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