# Linux-Snip-Tool
Snip tool for Linux environment.
Written entirely in Rust using xcb for window creation and image data, and xclip to paste the image data to the clipboard.

![](https://github.com/hambrickna/Linux-Snip-Tool/blob/main/SnipDemo.gif)

## Prerequisites

On Linux you need the xclip library, install it with something like: 

```bash
sudo apt-get install xclip
```

You will also need to have both Rust and Rust's package manager Cargo to run the install script
Install both with something like:

```bash
sudo curl https://sh.rustup.rs -sSf | sh
```

Finally, you will need some additional libxcb dependencies.  You can simply install with something like:

```bash
sudo apt-get install libx11-xcb-dev
```bash





