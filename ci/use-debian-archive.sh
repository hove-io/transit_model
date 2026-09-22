#!/bin/sh
# Redirect Debian 11 "bullseye" APT repositories to archive.debian.org.
#
# Debian 11 reached end-of-life: deb.debian.org and the country mirrors no longer
# serve bullseye, and its Release files are frozen with an expired "Valid-Until".
#
# This script, on Debian 11 only:
#   - repoints the main archive (incl. bullseye-updates) to archive.debian.org;
#   - drops the existing bullseye-security entries and replaces them with a
#     pinned snapshot.debian.org mirror -- archive.debian.org does not serve
#     bullseye-security, but snapshot.debian.org does, frozen at a fixed date;
#   - adds debian.ethz.ch / ftp.gwdg.de as HTTPS fallback mirrors for
#     main/updates -- archive.debian.org is known to be flaky/rate-limited;
#     this needs ca-certificates, bootstrapped first via archive.debian.org
#     (plain HTTP, no certs required);
#   - disables the Valid-Until check so `apt-get update` accepts the frozen
#     Release files.
#
# It is a no-op on any other Debian/Ubuntu release, so it can be called
# unconditionally from CI steps and Dockerfiles.
set -eu

# Date of the snapshot.debian.org security snapshot to pin to. Bump when more
# recent security fixes are needed.
SNAPSHOT_SECURITY_DATE="20260902T000000Z"

codename=""
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    codename="${VERSION_CODENAME:-}"
fi

if [ "$codename" != "bullseye" ]; then
    echo "use-debian-archive: not Debian 11 (codename='${codename:-unknown}'), nothing to do."
    exit 0
fi

echo "use-debian-archive: redirecting bullseye APT sources to archive.debian.org"

export DEBIAN_FRONTEND=noninteractive
export APT_LISTCHANGES_FRONTEND=none

# One-line sources (`deb http://<mirror>.debian.org/debian[-security] <suite> ...`).
for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.list; do
    [ -f "$f" ] || continue
    sed -i -E \
        -e '\#https?://[A-Za-z0-9.-]+\.debian\.org/debian-security#d' \
        -e '/[[:space:]]bullseye-security([[:space:]]|$)/d' \
        -e 's#https?://[A-Za-z0-9.-]+\.debian\.org/debian#http://archive.debian.org/debian#g' \
        "$f"
done

# archive.debian.org does not serve bullseye-security: pin a snapshot.debian.org
# mirror instead, so security updates stay installable.
echo "deb http://snapshot.debian.org/archive/debian-security/${SNAPSHOT_SECURITY_DATE} bullseye-security main" \
    > /etc/apt/sources.list.d/snapshot-security.list

# Frozen Release files carry an expired "Valid-Until".
printf 'Acquire::Check-Valid-Until "false";\nAcquire::Retries "5";\n' > /etc/apt/apt.conf.d/99archive-no-check-valid

# Bootstrap ca-certificates via archive.debian.org (plain HTTP, no certs
# needed) so the HTTPS fallback mirrors below can be verified.
apt-get update
apt-get install --yes --no-install-recommends ca-certificates

# archive.debian.org is known to be flaky/rate-limited: add HTTPS mirrors as
# fallback for main/updates.
cat > /etc/apt/sources.list.d/alternate-mirror.list <<EOF
deb https://debian.ethz.ch/debian bullseye main
deb https://debian.ethz.ch/debian bullseye-updates main
deb https://ftp.gwdg.de/debian bullseye main
deb https://ftp.gwdg.de/debian bullseye-updates main
EOF
apt-get update

echo "use-debian-archive: done"
