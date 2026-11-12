# The rws-mesh-init image: iptables and the script setting a meshed pod's
# rules (Gateway API mesh profile). Built per release as
# ghcr.io/bohdaq/rws-mesh-init:<version>.
FROM alpine:3.22
RUN apk add --no-cache iptables ip6tables
COPY rws-mesh-init.sh /usr/local/bin/rws-mesh-init
ENTRYPOINT ["/usr/local/bin/rws-mesh-init"]
