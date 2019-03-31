#!/bin/sh
#pkg install gcc8
#pkg install gpg

#git clone https://github.com/tormol/posixmq
#cd posixmq
#git remote add torbmol https://torbmol.no/code/torbmol/posixmq

# add cc command used by libc
echo -n '#!/bin/bash\nexec gcc $@' > /usr/bin/cc
chmod a+x /usr/bin/cc
# or ln -s

# From https://pkgsrc.joyent.com/install-on-illumos/
BOOTSTRAP_TAR="bootstrap-trunk-x86_64-20190317.tar.gz"
BOOTSTRAP_SHA="cda0f6cd27b2d8644e24bc54d19e489d89786ea7"

# Download the bootstrap kit to the current directory.
curl -O https://pkgsrc.joyent.com/packages/SmartOS/bootstrap/${BOOTSTRAP_TAR}

# Verify the SHA1 checksum.
#[ "${BOOTSTRAP_SHA}" = "$(/bin/digest -a sha1 ${BOOTSTRAP_TAR})" ] || echo "ERROR: checksum failure"

# Verify PGP signature.  This step is optional, and requires gpg.
curl -O https://pkgsrc.joyent.com/packages/SmartOS/bootstrap/${BOOTSTRAP_TAR}.asc
curl -sS https://pkgsrc.joyent.com/pgp/DE817B8E.asc | gpg --import
#gpg --verify ${BOOTSTRAP_TAR}{.asc,}

# Install bootstrap kit to /opt/local
tar -zxpf ${BOOTSTRAP_TAR} -C /

# Add to PATH/MANPATH.
PATH=/opt/local/sbin:/opt/local/bin:$PATH
MANPATH=/opt/local/man:$MANPATH
