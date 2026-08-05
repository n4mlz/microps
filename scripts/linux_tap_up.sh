#!/usr/bin/env bash

set -euo pipefail

tap_name="microps0"
tap_address="10.0.0.1/24"
tap_network="10.0.0.0/24"

if [[ ${EUID} -ne 0 ]]; then
    exec sudo "$0"
fi

tap_user="${SUDO_USER:-$(id -un)}"
external_device="$(ip route show default | awk 'NR == 1 { print $5 }')"

if [[ -z ${external_device} ]]; then
    printf 'default route device not found\n' >&2
    exit 1
fi

if ! ip link show dev "$tap_name" >/dev/null 2>&1; then
    ip tuntap add dev "$tap_name" mode tap user "$tap_user"
fi

ip addr replace "$tap_address" dev "$tap_name"
ip link set dev "$tap_name" up

echo 1 > /proc/sys/net/ipv4/ip_forward
iptables -A FORWARD -o "$tap_name" -j ACCEPT
iptables -A FORWARD -i "$tap_name" -j ACCEPT
iptables -t nat -A POSTROUTING -s "$tap_network" -o "$external_device" -j MASQUERADE

printf 'TAP %s is up (%s, owner %s)\n' "$tap_name" "$tap_address" "$tap_user"
