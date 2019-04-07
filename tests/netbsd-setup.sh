#!/bin/sh

# During install:
# * configure network
# * maybe setup ssh

# package repository is for some reason not configured by default ...
ARCH=`uname -m`
OS=`uname -s`
VERSION=`uname -r`
export PKG_PATH="http://ftp.netbsd.org/pub/pkgsrc/packages/$OS/$ARCH/$VERSION/All"
# or if typing, probably:
#export PKG_PATH="http://ftp.netbsd.org/pub/pkgsrc/packages/NetBSD/amd64/8.0/All"

# install TLS certificates to download stuff via HTTPS
pkg_add -v mozilla-rootcerts
mozilla-rootcerts install

# Install Rust
pkg_add -v curl # FIXME maybe there is an already installed tool I could use
curl https://sh.rustup.rs -sSf --output rustup.sh
# chmod a+x rustup.sh && ./rustup.sh doesn't work for some reason
sh rustup.sh -y
. ~/.cargo/env

pkg_add -v nano # I'm no good at vi

# clone repo
pkg_add -v git
git clone https://github.com/tormol/posixmq
cd posixmq

# test
cargo test -- --skip receive_errors # this test triggers EMFILE bug
cargo test -- --ignored
