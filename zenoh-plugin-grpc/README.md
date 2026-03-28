# zenoh-plugin-grpc

Zenoh `1.7.2` gRPC plugin.

## Build

From the workspace root:

```bash
cargo build -p zenoh-plugin-grpc
```

The dynamic library will be produced under:

```bash
target/debug/libzenoh_plugin_grpc.so
```

The exact filename depends on the OS.

## Configure In `zenohd`

Example `zenoh.json5`:

```json5
{
  mode: "router",
  plugins_loading: {
    enabled: true,
    search_dirs: ["./target/debug"],
  },
  plugins: {
    grpc: {
      host: "127.0.0.1",
      port: 7335,
      uds_path: "/tmp/zenoh-grpc.sock",
    },
  },
}
```

Then start:

```bash
zenohd -c zenoh.json5
```

## Default Listen Address

- TCP: `127.0.0.1:7335`
- UDS: `/tmp/zenoh-grpc.sock`
