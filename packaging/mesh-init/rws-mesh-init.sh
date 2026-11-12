#!/bin/sh
# rws-mesh-init: the iptables rules of a meshed pod (Gateway API mesh
# profile, spec/GATEWAY_MESH.md). The pod's outbound TCP goes to the rws
# sidecar's port, except the sidecar's own (by UID) and loopback; inbound
# traffic is untouched. Runs once, as root with NET_ADMIN and NET_RAW.
set -eu
PORT="${RWS_MESH_OUTBOUND_PORT:-15001}"
SIDECAR_UID="${RWS_MESH_UID:-1337}"

rules() {
  ipt="$1"
  "$ipt" -t nat -N RWS_MESH_OUTPUT 2>/dev/null || "$ipt" -t nat -F RWS_MESH_OUTPUT
  "$ipt" -t nat -A RWS_MESH_OUTPUT -m owner --uid-owner "$SIDECAR_UID" -j RETURN
  "$ipt" -t nat -A RWS_MESH_OUTPUT -o lo -j RETURN
  "$ipt" -t nat -A RWS_MESH_OUTPUT -p tcp -j REDIRECT --to-ports "$PORT"
  "$ipt" -t nat -C OUTPUT -p tcp -j RWS_MESH_OUTPUT 2>/dev/null || "$ipt" -t nat -A OUTPUT -p tcp -j RWS_MESH_OUTPUT
}

rules iptables
echo "rws-mesh-init: IPv4 outbound TCP -> :$PORT (except UID $SIDECAR_UID, loopback)"
# IPv6 only where the pod has it.
if [ -e /proc/net/if_inet6 ] && rules ip6tables 2>/dev/null; then
  echo "rws-mesh-init: IPv6 likewise"
fi
