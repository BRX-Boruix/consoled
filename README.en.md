# consoled

BORUIX's **terminal byte producer daemon**: it reads the keyboard event stream, converts it into bytes, and writes them to the console device.

[简体中文](README.md)

## The data path

```
keyboard event stream → keymap (to bytes) → console device
```

The process is a **resident daemon**: no arguments, never exits (it exits only if assembly fails).

## Why the conversion lives in user space

The keymap is **policy, not mechanism**. The kernel's standard input keeps a **raw byte stream**, and layout, escaping, and control-key folding all move up into this process. Changing the mapping rules then requires no kernel change.

## Behaviour on failure

| Situation | Handling |
| --- | --- |
| The event stream or console cannot be opened | Report honestly and **exit non-zero** (an assembly error, not silent) |
| The event read returns the "try again later" sentinel | **Carry on unchanged** — it is a retry signal, neither an error nor end of file |
| The console buffer is full | **Discard the remaining bytes and count them**; **never retry** |
| Any other read error | Report honestly and exit — a broken ring is a kernel defect and soldiering on is pointless |

**"Never retry" deserves a note**: this process is single-threaded. A full buffer means the reader has not consumed anything for 100 ms or more — and human typing is far slower than the drain rate of a 4 KiB ring, so that shape appears only when **there is no reader**. Retrying in place would then **starve the event-reading side**, to the point of not receiving keyboard input at all. Discarding and counting is the only choice that does not deadlock itself.

## Observability

Every 256 events it prints one line of statistics to the serial port, including the discard count, so the state can be observed without interrupting operation.

## Building

```bash
cargo build --release
```

Started by the system init process at boot, then resident.

## Layout

```
consoled/
├── Cargo.toml    # package definition
├── build.rs      # injects the linker script
├── linker.ld     # user-space section layout
└── src/
    └── main.rs   # event reading, conversion, and writing
```

## Related projects

- [`consoled-e2e`](https://github.com/BRX-Boruix/consoled-e2e) — end-to-end acceptance for the console byte ring
- [`libsys`](https://github.com/BRX-Boruix/libsys) — provides the event reading and keymap interfaces
- [`libline`](https://github.com/BRX-Boruix/libline) — provides the event source component

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
