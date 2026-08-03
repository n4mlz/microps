#!/usr/bin/env bash

set -euo pipefail

tap_name="microps0"

if [[ ${EUID} -ne 0 ]]; then
    exec sudo "$0"
fi

if ip link show dev "$tap_name" >/dev/null 2>&1; then
    ip addr flush dev "$tap_name"
    ip link set dev "$tap_name" down
    ip tuntap del dev "$tap_name" mode tap
fi

printf 'TAP %s is down\n' "$tap_name"
