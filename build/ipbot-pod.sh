#!/bin/sh

set -eo

if [[ "$#" -ne 1 ]]; then
  cat << EOF
This script supports only a single argument:
  - Telegram token string

Usage:
  ipbot-pod.sh "<telegram/token/string>"
EOF
  exit 1
fi

TOKEN="$1"

podman pod create --userns=keep-id -n ipbot-pod -p 7443:7443

# Podman 3 usage
# Volume mounting may be required to be updated to use :U flag, like
#   -v ipbot-vol:/run:U
# to properly tell a container to use correct UID:GID when accessing
# volume stuff

# Also if you use a webserver you may need to add a rule to allow
# input on 7443 port
# iptables:
# iptables -I INPUT -t tcp -i <docker/podman network iface> --dport 7443 -j ACCEPT
# nftables:
# nft add rule ip filter input iif <docker/podman network iface> tcp dport 7443 accept

podman create --pod ipbot-pod -t --restart unless-stopped \
    --name ipbot-db \
    -v $PWD/conf:/usr/local/etc/redis \
    -v ipbot-vol:/run \
    -v ipbot-vol:/var/lib/redis \
    docker.io/redis:7.0 \
    redis-server /usr/local/etc/redis/redis.conf

podman create -t --pod ipbot-pod --restart unless-stopped \
    --name ipbot \
    -v ipbot-vol:/run \
    -e RUST_LOG=debug \
    -e "TELOXIDE_TOKEN=$TOKEN" \
    -e "REDIS_SOCKET=redis+unix:///run/redis.sock" \
    docker.io/vpetrigo/ipbot:0.1.0 \
    /ipbot/ipbot -v
