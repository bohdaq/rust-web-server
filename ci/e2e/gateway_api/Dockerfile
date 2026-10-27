# The rws binary built on the CI runner, for the Gateway API conformance
# cluster (kind). Ubuntu 24.04 to match the runner's glibc.
FROM ubuntu:24.04
COPY rws /usr/local/bin/rws
WORKDIR /srv
EXPOSE 7878
ENTRYPOINT ["/usr/local/bin/rws"]
