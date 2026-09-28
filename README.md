# consoled

BORUIX 的控制台守护进程：把键盘事件流转成字节，写入控制台设备。

[English](README.en.md)

由系统初始化进程在启动时拉起，之后常驻运行，无参数。

## 它做什么

```
/devices/input/events → 按键解码 → /devices/console
```

- 读取键盘事件流，经按键映射转为字节流
- 支持修饰键：Shift、Ctrl、Alt 的状态跨事件批次保持
- 转换后的字节进入控制台，由终端会话读取

## 行为约定

- 启动时清空事件流中积压的按键：登录提示出现之前的键入不算数
- 控制台缓冲写满时丢弃剩余字节并计入统计，不重试——键盘输入速率远低于缓冲排空速率，堆积只发生在没有读者的时候
- 每处理 256 个事件输出一行统计：已处理事件数、已写字节数、丢弃字节数

## 已知限制

- 按键映射是固定的，没有可加载的键盘布局
- 实例号经命令行指定，缺省为 0；非法实例号会拒绝启动

## 构建

```bash
cargo build --release
```

## 文件结构

```
consoled/
├── Cargo.toml    # 包定义
├── build.rs      # 注入链接脚本
├── linker.ld     # 用户态段布局
└── src/
    └── main.rs   # 事件读取、按键解码与控制台写入
```

## 相关项目

- [`libsys`](https://github.com/BRX-Boruix/libsys) —— 事件读取与按键解码
- [`login`](https://github.com/BRX-Boruix/login) —— 登录提示的消费者
- [`shell`](https://github.com/BRX-Boruix/shell) —— 控制台字节的终端会话

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。
