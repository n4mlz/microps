#!/usr/bin/env bash

set -euo pipefail

tap_name="microps0"
tap_address="10.0.0.1/24"

if [[ ${EUID} -ne 0 ]]; then
    exec sudo "$0"
fi

tap_user="${SUDO_USER:-$(id -un)}"

if ! ip tuntap show dev "$tap_name" >/dev/null 2>&1; then
    ip tuntap add dev "$tap_name" mode tap user "$tap_user"
fi

ip addr replace "$tap_address" dev "$tap_name"
ip link set dev "$tap_name" up

printf 'TAP %s is up (%s, owner %s)\n' "$tap_name" "$tap_address" "$tap_user"
