# consoled

BORUIX's console daemon: turns the keyboard event stream into bytes and writes them to the console device.

[简体中文](README.md)

Started by the system init process at boot; runs for the lifetime of the system. Takes no arguments.

## What it does

```
/devices/input/events → key decoding → /devices/console
```

- Reads the keyboard event stream and converts it to a byte stream through the key map
- Tracks modifier keys: Shift, Ctrl and Alt may be pressed and released across separate reads, with the state preserved
- Converted bytes enter the console, where terminal sessions read them

## Behaviour

- On startup it drains keystrokes accumulated in the event stream: keys typed before the login prompt appears do not count
- When the console buffer is full the remaining bytes are dropped and counted, never retried — typing is far slower than the buffer drains, so pile-up only happens when no reader exists
- Once every 256 events it prints one statistics line: events processed, bytes written, bytes dropped

## Known limitations

- The key map is fixed; there are no loadable layouts
- The instance id comes from the command line and defaults to 0; an invalid instance id refuses to start

## Building

```bash
cargo build --release
```

## Repository layout

```
consoled/
├── Cargo.toml    # package manifest
├── build.rs      # injects the linker script
├── linker.ld     # user-space segment layout
└── src/
    └── main.rs   # event reading, key decoding, console writing
```

## Related projects

- [`libsys`](https://github.com/BRX-Boruix/libsys) — event reading and key decoding
- [`login`](https://github.com/BRX-Boruix/login) — consumer of the login prompt
- [`shell`](https://github.com/BRX-Boruix/shell) — terminal session for the console bytes

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
