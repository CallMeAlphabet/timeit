# timeit

A simple, precise command timing utility written in Rust. 
Because I like showing off how fast my hex dumping tool, [`fasthex`](https://github.com/CallMeAlphabet/fasthex/), and others are.

## Table of Contents

- [Quick Start](#quick-start)
- [Uninstall](#Uninstall)
- [Help Message](#help-message)

## Quick Start

- **On Arch**
```bash
paru -S timeit 
# or timeit-bin for a prebuilt release
```
- **Non-arch**
```bash
cargo install timeit-cli
```
## Uninstall

```bash
cargo uninstall timeit
```

## Help Message

```
❯ timeit --help
timeit - precise command timing utility

USAGE:
    timeit [OPTIONS] <command> [args...]
    timeit --compare "<command1>" "<command2>"

FLAGS:
    -q, --quiet              Only show average (suppress per-run output)
    -w, --warmup N           Run N warmup iterations (5s cooldown after)
    -r, --runs N             Number of measured runs (default: 1)
    -p, --profile=NAME       Load profile from ~/.local/bin/timeit.d/NAME.profile
    -h, --help               Show this help
    --median                 Show median instead of average
    --timeout DURATION       Kill commands after timeout (e.g., 30s, 5m, 100ms)
    --compare                Compare two commands (requires two quoted commands)

DISPLAY MODES:
    --seconds                Show time in decimal seconds only (e.g., 70.982441221)
    --millis                 Show time in decimal milliseconds (e.g., 70982.441221)
    --micros                 Show time in decimal microseconds (e.g., 70982441.221)
    --nanos                  Show time in integer nanoseconds (e.g., 70982441221)
    (default: auto-format with smart units, e.g., 1m 10s 982ms 441μs 221ns)

EXAMPLES:
    timeit fasthex file.bin
    timeit -r 10 --median fasthex file.bin
    timeit -w 3 -r 10 --timeout 30s fasthex file.bin
    timeit -q -r 100 fasthex file.bin
    timeit --profile=bench fasthex file.bin
    timeit --compare "hexdump file.bin" "fasthex file.bin"
    timeit --timeout 5m long-running-cmd

TIMEOUT UNITS:
    ns, μs/us, ms, s, m, h (e.g., 100ms, 30s, 5m, 1h)

PROFILES:
    Create profile files in ~/.local/bin/timeit.d/ with format:
    
    runs=100
    warmup=5
    quiet=true
```

