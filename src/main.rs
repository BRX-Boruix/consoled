//! BORUIX `consoled`：用户态终端字节生产者（I-EVENTS 阶段 3 · 甲-a，ADR-045）。
//!
//! **职责（§6.15 P3，单径）**：读键盘事件流 `/devices/input/events` → 经 libsys
//! keymap（`feed`/`decode_into`，内核 KEYMAP 的逐字节用户态镜像）转成字节 →
//! 写入 console 设备 `/devices/console`（P2 节点，SPSC 环 + 等待者唤醒）。
//! 本进程是**常驻守护进程**：无参数、不退出（exit 只在装配失败时发生）。
//!
//! **为何转换在用户态**（ADR-045 裁决甲-a）：keymap 是策略而非机制；内核
//! stdin 保持原始字节流（ADR-045 决策 3），布局/转义/Ctrl 折叠全部上移。
//!
//! **与旧轨道 A 的关系**：P4 切换前，IRQ1 字节译码（内核 KBD 路径）与本项目
//! **双轨并存**——P1 的每读者事件流保证二者互不消费对方的数据。本进程上线
//! 不改变任何现有行为（事件流此前无读者时积压可回收；现在 consoled 是第一位
//! 常驻读者）。
//!
//! **失败模式（S20，先于正常路径）**：
//! - 事件流/console 打不开 → 如实报错并**退出非零**（装配错误，不静默）；
//! - 事件读的阻塞-唤醒哨兵（WouldBlock）→ **原样继续**（libsys/event.rs
//!   「两次实测缺陷」的成文契约：哨兵 = 重试信号，不是错误也不是 EOF）；
//! - console 环满短写（n < len）→ **丢剩余字节 + dropped 计数**：环满说明
//!   读者（P4 后的 shell stdin）100ms 级没消费——人手键入速率远低于 4KiB
//!   环的排空速率，此形态只出现在「读者不存在」（P4 前的正常态）。
//!   **绝不重试**：consoled 单线程，重试会饿死事件读侧（audiod 同款教训）；
//! - 非 WouldBlock 的读错误 → 如实报错退出（环坏 = 内核缺陷，硬扛无意义）。
//!
//! **可观测性（S09）**：周期性（每 256 事件）向串口 stdout 打一行统计；
//! 遥测计数器（drecs/dbytes）经 `debug_read_stats` 汇报，与 evsrcdemo 同款。
#![no_std]
#![no_main]
extern crate alloc;

use libsys::event::{decode_into, ConsoleWriter, EventSourceReader, KeymapState};
use libsys::{write, STDOUT};


/// 统计行打印周期（按键事件数）。太密刷屏串口（putc_wait 洪泛丢字节的
/// 已知缺陷），太疏失去可观测性——256 事件 ≈ 半屏输入，实测折中。
const STATS_EVERY: u64 = 256;

fn out(b: &[u8]) {
    let _ = write(STDOUT, b);
}

fn outln(b: &[u8]) {
    out(b);
    out(b"\n");
}

fn out_u64(mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if v == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out(&buf[i..]);
}

/// 解析 argv[0] 为实例 id（十进制；无 argv = 0——兼容既有 spawn 形态）。
/// 非法（非数字/负号/超长）如实拒绝返回 None，由调用方装配失败退出——
/// 绝不静默夹到实例 0（S09/S17：错误的实例号 = 写错终端，比死更糟）。
fn parse_instance(argc: isize, argv: *const *const u8) -> Option<usize> {
    if argc <= 0 || argv.is_null() {
        return Some(0);
    }
    // SAFETY: argc>=1 且 argv 由内核 exec 路径按 C 数组构造（NUL 结尾，
    // init/src/main.rs 同款访问形态）。
    let p = unsafe { *argv } ;
    if p.is_null() {
        return Some(0);
    }
    let mut n: usize = 0;
    let mut i = 0isize;
    let mut any = false;
    unsafe {
        while *p.offset(i) != 0 {
            let c = *p.offset(i);
            if c < b'0' || c > b'9' {
                return None;
            }
            n = n.checked_mul(10)?.checked_add((c - b'0') as usize)?;
            any = true;
            i += 1;
        }
    }
    if !any { return None; }
    Some(n)
}

#[unsafe(no_mangle)]
pub extern "C" fn user_main(argc: isize, argv: *const *const u8) -> i32 {
    let instance = match parse_instance(argc, argv) {
        Some(v) => v,
        None => {
            outln(b"[consoled] FATAL: bad instance id in argv");
            return 1;
        }
    };
    outln(b"[consoled] starting (events -> keymap -> console instance)");

    // ---- 装配：两个端都必须成功；任一失败 = 装配错误，如实退出（S09）----
    let mut ev = match EventSourceReader::open() {
        Ok(r) => r,
        Err(_) => {
            outln(b"[consoled] FATAL: open(/devices/input/events) failed");
            return 1;
        }
    };
    let con = match ConsoleWriter::open_instance(instance) {
        Ok(w) => w,
        Err(_) => {
            outln(b"[consoled] FATAL: open console write end failed (instance above)");
            return 1;
        }
    };
    outln(b"[consoled] both ends open; serving");

    // ---- getty 清积压（getty/login 的 tcflush(TCIFLUSH) 同构，2026-09-27）----
    // 失败模式（S20）：login 尚未启动时用户按键会积压在事件环（无读者消费）。
    // consoled 首次服务若照单全收转发进实例环，login 起来后**整段积压一次性
    // 重放**——用户见「按键没反应→重按→开机后回显成串重复」（实测复现：
    // login 前键入 rrrooo，login 提示出现后回显 rr/rrr/…/rrrooo 逐次重绘）。
    // Unix 同构语义：getty 打开 tty 时清输入缓冲——登录提示之前的按键不算数。
    // 实现：非阻塞排空事件环、**直接丢弃**（不转换、不写环）。仅推进本读者
    // 游标（广播模型），不碰其他实例/读者（S15：清的是「本实例的输入历史」）。
    // respawn 场景同语义：新守护接管时，守护空窗期的按键一并作废（与 tty
    // respawn 后 getty flush 行为一致）。读空即止（WouldBlock = 环已空）。
    {
        let mut scratch = [0u8; 128];
        let mut flushed: usize = 0;
        loop {
            match libsys::read_nonblocking_take(ev.raw_fd(), &mut scratch) {
                Ok(0) => break,
                Ok(n) => flushed += n,
                Err(libsys::Error::WouldBlock) => break,
                Err(_) => break,
            }
        }
        if flushed > 0 {
            outln(b"[consoled] flushed stale input backlog (pre-login keystrokes)");
        }
    }

    let mut raw = alloc::vec::Vec::new();
    let mut bytes_out = alloc::vec::Vec::new();
    // keymap 修饰键状态机跨轮持有（Shift 按下→抬起可能分属两轮——
    // decode_into 状态存活的契约，libsys 宿主测试钉过）。
    let mut keymap = KeymapState::default();
    let mut recs_total: u64 = 0;
    let mut bytes_total: u64 = 0;
    let mut dropped_total: u64 = 0;

    loop {
        // ---- 1. 阻塞读一批事件（环空时内核挂起本进程——不自旋）----
        raw.clear();
        match ev.read_records(&mut raw) {
            Ok(_) => {}
            // 阻塞-唤醒哨兵：**原样继续**（成文契约：哨兵 = 立即重试信号）。
            Err(libsys::Error::WouldBlock) => {
                let _ = libsys::yield_now();
                continue;
            }
            Err(_) => {
                outln(b"[consoled] FATAL: event read failed (non-sentinel)");
                let _ = ev.close();
                let _ = con.close();
                return 2;
            }
        }

        // ---- 2. 事件 → 字节（唯一转换点：libsys decode_into，S13）----
        bytes_out.clear();
        let rr = decode_into(&mut keymap, &raw, &mut bytes_out);

        // ---- 3. 字节 → console 环（短写 = 丢剩余，绝不重试）----
        if !bytes_out.is_empty() {
            let n = match con.write(&bytes_out) {
                Ok(n) => n,
                Err(_) => {
                    outln(b"[consoled] FATAL: console write failed");
                    let _ = ev.close();
                    let _ = con.close();
                    return 3;
                }
            };
            bytes_total += n as u64;
            dropped_total += (bytes_out.len() - n) as u64;
        }

        // ---- 4. 周期统计（可观测；不伪造，只报真计数）----
        // 越界判定：recs_total 跨过 STATS_EVERY 的整数倍才打印——与「每轮
        // 恰好读满 8 条」的缓冲尺寸无隐式耦合（短读/空轮都不影响节奏）。
        let prev = recs_total;
        recs_total += rr.count as u64;
        if recs_total / STATS_EVERY > prev / STATS_EVERY {
            out(b"[consoled] recs=");
            out_u64(recs_total);
            out(b" bytes=");
            out_u64(bytes_total);
            out(b" dropped=");
            out_u64(dropped_total);
            outln(b"");
        }
    }
}
