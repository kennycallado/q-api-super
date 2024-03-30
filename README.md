
## DEV:
https://docs.rs/surrealdb/latest/surrealdb/engine/local/index.html

``` bash
nix develop github:surrealdb/surrealdb --offline
```

## BUILD:

### cross:

``` bash
nix-shell -p rustup cargo-cross
```

``` bash
cross build --target aarch64-unknown-linux-musl
```
