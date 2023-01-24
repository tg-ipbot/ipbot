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
