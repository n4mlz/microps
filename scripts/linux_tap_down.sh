#!/usr/bin/env bash

set -euo pipefail

tap_name="microps0"
tap_network="10.0.0.0/24"

if [[ ${EUID} -ne 0 ]]; then
    exec sudo "$0"
fi

external_device="$(ip route show default | awk 'NR == 1 { print $5 }')"

if [[ -n ${external_device} ]]; then
    iptables -t nat -D POSTROUTING -s "$tap_network" -o "$external_device" -j MASQUERADE || true
fi
iptables -D FORWARD -i "$tap_name" -j ACCEPT || true
iptables -D FORWARD -o "$tap_name" -j ACCEPT || true
echo 0 > /proc/sys/net/ipv4/ip_forward

if ip link show dev "$tap_name" >/dev/null 2>&1; then
    ip addr flush dev "$tap_name"
    ip link set dev "$tap_name" down
    ip tuntap del dev "$tap_name" mode tap
fi

printf 'TAP %s is down\n' "$tap_name"
