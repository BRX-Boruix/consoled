# consoled

**简体中文** | [English](#english)

BORUIX 的**用户态终端字节生产者**——把键盘事件转换成终端字节，写入 console 设备。

```
/devices/input/events → 键码转换（含修饰键）→ /devices/console
```

`consoled` 是一个**常驻守护进程**：启动后一直运行，正常情况下不退出。

---

## 它做什么

它读取内核的键盘事件流，把事件转换成终端字节（含布局、转义、Ctrl 组合键的折叠），再写入
console 设备。console 设备的另一端由 shell 之类的读者消费，最终变成用户看到的输入。

**为什么转换发生在用户态**：键码到字节的映射是**策略，不是机制**。内核只负责把原始按键事件
交给用户态，布局与转义规则留在用户态，就可以在不改内核的前提下调整。内核的 stdin 因此保持
原始字节流。

## 实例号

程序接受一个可选的实例号作为参数：

```
consoled        # 实例 0
consoled 1      # 实例 1
```

这用于支持多个终端实例。参数非法（不是数字、超长、溢出）时会**如实报错并退出**，绝不静默
退回实例 0——写错终端比直接失败糟糕得多。

## 启动时清空积压

程序启动时会先**非阻塞地排空**事件环并直接丢弃，然后才开始正常服务。

原因是 Unix 上 `getty` 的同类行为：登录提示出现之前用户敲的键不该算数。如果不清理，登录
程序启动后会把登录之前积压的按键**一次性重放**出来——用户会看到"敲了没反应、于是重敲、
登录后一下子冒出一串重复字符"。

清理只推进本实例自己的读取位置，不影响其他实例或读者。

## 缓冲满时丢弃而不重试

写入 console 环时若空间不足（短写），剩余字节**直接丢弃并计数**，不重试。

原因是 `consoled` 是单线程的：在写端重试会饿死事件读取那一侧。而人手键入的速率远低于缓冲
的排空速率，只有"读者根本不存在"时才会出现写满——那种情况下重试也没有意义。

丢弃的字节数会如实计入统计并打印，不会静默吞掉。

## 可观测性

每处理 256 个事件，程序向标准输出打印一行统计：

```
[consoled] recs=<事件数> bytes=<已写字节数> dropped=<丢弃字节数>
```

## 出错时的行为

| 情况 | 行为 |
| --- | --- |
| 事件流或 console 打不开 | 如实报错并**退出**（装配失败，不静默继续） |
| 事件读取返回"曾阻塞、请重试" | **正常路径**，立即重试（不是错误） |
| console 环满 | 丢弃剩余字节并计数（见上） |
| 其他读错误 | 如实报错并退出 |

## 构建

```bash
cargo build --release
```

编译产物部署为 BORUIX 系统中的用户态程序，由 `init` 启动。

## 文件结构

```
consoled/
├── Cargo.toml    # 包定义
├── build.rs      # 注入链接脚本
├── linker.ld     # 用户态段布局
└── src/
    └── main.rs   # 程序本体
```

## 相关项目

- [`consoled-e2e`](https://github.com/BRX-Boruix/consoled-e2e) —— 本程序的端到端验收
- [`libsys`](https://github.com/BRX-Boruix/libsys) —— 用户态系统调用封装，含事件转换层

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。

---

# English

[简体中文](#consoled) | **English**

The BORUIX **user-space terminal byte producer** — it turns keyboard events into terminal bytes and
writes them to the console device.

```
/devices/input/events → keycode translation (incl. modifiers) → /devices/console
```

`consoled` is a **long-running daemon**: once started it keeps serving and does not exit under
normal operation.

---

## What it does

It reads the kernel's keyboard event stream, translates events into terminal bytes (covering layout,
escape sequences, and Ctrl combination folding), and writes them to the console device. The other
end of that device is consumed by a reader such as the shell, eventually becoming the input the user
sees.

**Why translation happens in user space**: mapping keycodes to bytes is **policy, not mechanism**.
The kernel only needs to hand raw key events to user space; keeping layout and escape rules there
means they can be adjusted without touching the kernel. The kernel's stdin therefore stays a raw
byte stream.

## Instance number

The program accepts an optional instance number as its argument:

```
consoled        # instance 0
consoled 1      # instance 1
```

This supports multiple terminal instances. An invalid argument (non-numeric, overlong, or
overflowing) is **reported and exits** — it never silently falls back to instance 0, because
writing to the wrong terminal is far worse than failing outright.

## Clearing the backlog at startup

At startup the program **non-blockingly drains** the event ring and discards what it finds, before
beginning normal service.

The reason mirrors `getty` behaviour on Unix: keystrokes typed before the login prompt appears
should not count. Without this, the login program would **replay the entire pre-login backlog** at
once, and the user would see "I typed and nothing happened, so I typed again — then a burst of
repeated characters appeared after login".

The drain only advances this instance's own read position; it does not affect other instances or
readers.

## Dropping rather than retrying when the buffer is full

If the console ring has insufficient space (a short write), the remaining bytes are **dropped and
counted**, not retried.

The reason is that `consoled` is single-threaded: retrying on the write side would starve the event
reading side. Human typing is far slower than the buffer drains, so a full buffer only occurs when
the reader does not exist at all — and in that case retrying would be pointless anyway.

Dropped bytes are counted and reported honestly rather than silently swallowed.

## Observability

Every 256 events processed, the program prints one statistics line to standard output:

```
[consoled] recs=<events> bytes=<bytes written> dropped=<bytes dropped>
```

## Behaviour on failure

| Situation | Behaviour |
| --- | --- |
| Event stream or console cannot be opened | Report and **exit** (an assembly failure, never silently continue) |
| Event read returns "you blocked, retry" | **Normal path**; retry immediately (not an error) |
| Console ring full | Drop the remaining bytes and count them (see above) |
| Any other read error | Report and exit |

## Building

```bash
cargo build --release
```

The artifact is deployed as a user-space program in a BORUIX system and started by `init`.

## Layout

```
consoled/
├── Cargo.toml    # package definition
├── build.rs      # injects the linker script
├── linker.ld     # user-space section layout
└── src/
    └── main.rs   # the program itself
```

## Related projects

- [`consoled-e2e`](https://github.com/BRX-Boruix/consoled-e2e) — end-to-end acceptance for this program
- [`libsys`](https://github.com/BRX-Boruix/libsys) — the user-space syscall wrapper, including the event translation layer

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
