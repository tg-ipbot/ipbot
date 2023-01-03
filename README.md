## Redis test container run

- Podman option:

```bash
podman run --userns keep-id -it --rm -p 6379:6379 \
  -v $PWD/conf:/usr/local/etc/redis -v $PWD/run:/run \
  docker.io/redis:7.0 redis-server /usr/local/etc/redis/redis.conf
```

## License

This project is licensed under:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as
defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
