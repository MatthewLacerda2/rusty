#!/usr/bin/env bash
# Install apt packages from a directory of cached `.deb`s, going to Ubuntu's
# mirror only when the cache missed (#2101). `action.yml` beside this file says
# why; `.github/scripts/tests/test_system_libs.py` runs this script against a
# fake apt and dpkg.
#
#   install.sh <archive-dir> <cache-hit: true|false> <package>...
#
# - A hit installs the directory's `.deb`s with `dpkg -i` and never calls
#   apt-get: no `update`, no network. If that install fails (a corrupt entry),
#   it says so and falls through to the miss path.
# - A miss runs `apt-get update` and a `--download-only` install into the
#   directory, each under `timeout` and with `Acquire::Retries`, the pair tried
#   `ATTEMPTS` times `PAUSE` seconds apart; then installs from what it fetched
#   with `--no-download`. The directory then holds exactly the `.deb`s this
#   image lacked, which `action.yml` saves.
# - After a failed attempt the image's Azure mirror is swapped for Ubuntu's own
#   archive in `APT_SOURCES` (the runner's mirror list and sources files): on
#   this PR's first CI run, 2026-11-11 15:33 UTC, `azure.archive.ubuntu.com`
#   timed out on every index for ten minutes, so retrying it alone could not
#   pass. Timeouts are short so a dead mirror costs minutes, not a quarter-hour.
# - Either way it ends by asking dpkg whether every package asked for is
#   installed, and fails naming the ones that are not: a mirror that answers
#   `update` with warnings only (apt exits 0 on a failed fetch) or a half-done
#   install never passes silently.
set -euo pipefail

dir=$1 hit=$2
shift 2
packages=("$@")
attempts=${ATTEMPTS:-3}
pause=${PAUSE:-20}
apt=(apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=20 -o "Dir::Cache::archives=$dir")
# shellcheck disable=SC2206 # a glob list, split on purpose
sources=(${APT_SOURCES:-/etc/apt/apt-mirrors.txt /etc/apt/sources.list /etc/apt/sources.list.d/*})

mkdir -p "$dir/partial"

from_cache() {
  local debs=("$dir"/*.deb)
  if [ ! -e "${debs[0]}" ]; then
    echo "system-libs: cache hit, nothing to install (the image has every package)"
    return 0
  fi
  echo "system-libs: cache hit, installing ${#debs[@]} .deb(s) offline"
  sudo dpkg -i "${debs[@]}"
}

to_ubuntu_archive() {
  local file
  for file in "${sources[@]}"; do
    [ -f "$file" ] && sudo sed -i 's#azure\.archive\.ubuntu\.com#archive.ubuntu.com#g' "$file"
  done
  return 0
}

from_mirror() {
  local try
  for try in $(seq 1 "$attempts"); do
    if sudo timeout 180 "${apt[@]}" update \
      && sudo timeout 600 "${apt[@]}" install -y --download-only "${packages[@]}"; then
      sudo "${apt[@]}" install -y --no-download "${packages[@]}"
      return 0
    fi
    if [ "$try" -lt "$attempts" ]; then
      echo "system-libs: the mirror failed (attempt $try of $attempts); again in ${pause} s from archive.ubuntu.com"
      to_ubuntu_archive
      sleep "$pause"
    fi
  done
  echo "::error::system-libs: could not download ${packages[*]} from the mirror after $attempts attempts, and the cache had no entry for this image"
  return 1
}

if [ "$hit" = true ]; then
  from_cache || { echo "::warning::system-libs: the cached .debs did not install; going to the mirror"; rm -f "$dir"/*.deb; from_mirror; }
else
  from_mirror
fi

missing=()
for package in "${packages[@]}"; do
  status=$(dpkg-query -W -f='${db:Status-Status}' "$package" 2>/dev/null || true)
  [ "$status" = installed ] || missing+=("$package")
done
if [ ${#missing[@]} -gt 0 ]; then
  echo "::error::system-libs: not installed after the install: ${missing[*]}"
  exit 1
fi

# What `actions/cache/save` keeps: the `.deb`s, owned by the runner's user.
sudo rm -rf "$dir/partial" "$dir/lock"
sudo chown -R "$(id -u):$(id -g)" "$dir"
# Never an empty directory, which the cache would refuse to save.
printf '%s\n' "${packages[@]}" > "$dir/packages.txt"
echo "system-libs: installed ${packages[*]}"
