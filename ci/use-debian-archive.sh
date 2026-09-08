#!/bin/sh
# Redirect Debian 11 "bullseye" APT repositories to archive.debian.org.
#
# Debian 11 reached end-of-life: deb.debian.org and the country mirrors no longer
# serve bullseye, and its Release files are frozen with an expired "Valid-Until".
#
# This script, on Debian 11 only:
#   - repoints the main archive (incl. bullseye-updates) to archive.debian.org;
#   - drops the bullseye-security entries -- security is NOT published on
#     archive.debian.org for bullseye, and it is EOL anyway;
#   - disables the Valid-Until check so `apt-get update` accepts the frozen
#     Release files.
#
# It is a no-op on any other Debian/Ubuntu release, so it can be called
# unconditionally from CI steps and Dockerfiles.
set -eu

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

# One-line sources (`deb http://<mirror>.debian.org/debian[-security] <suite> ...`).
for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.list; do
    [ -f "$f" ] || continue
    sed -i -E \
        -e '\#https?://[A-Za-z0-9.-]+\.debian\.org/debian-security#d' \
        -e '/[[:space:]]bullseye-security([[:space:]]|$)/d' \
        -e 's#https?://[A-Za-z0-9.-]+\.debian\.org/debian#http://archive.debian.org/debian#g' \
        "$f"
done

# deb822 sources (`URIs:` / `Suites:` blocks). Rare on bullseye; handle the common
# single-URI case and always neutralise a security-only stanza.
for f in /etc/apt/sources.list.d/*.sources; do
    [ -f "$f" ] || continue
    if grep -qE 'debian-security|bullseye-security' "$f" && \
       ! grep -qE '\.debian\.org/debian([^-]|$)' "$f"; then
        # security-only file: disable it entirely
        : > "$f"
        continue
    fi
    sed -i -E \
        -e 's#https?://[A-Za-z0-9.-]+\.debian\.org/debian#http://archive.debian.org/debian#g' \
        -e 's#(^Suites:.*)[[:space:]]bullseye-security#\1#' \
        "$f"
done

# Frozen Release files carry an expired "Valid-Until".
printf 'Acquire::Check-Valid-Until "false";\n' > /etc/apt/apt.conf.d/99archive-no-check-valid

echo "use-debian-archive: done"
