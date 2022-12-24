## Redis test container run

- Podman option:

```bash
podman run --userns keep-id -it --rm -p 6379:6379 \
  -v $PWD/conf:/usr/local/etc/redis -v $PWD/run:/run \
  docker.io/redis:7.0 redis-server /usr/local/etc/redis/redis.conf
```