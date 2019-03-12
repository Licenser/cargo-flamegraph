# cargo-flamegraph

A simple cargo plugin that generates a flamegraph
for a given workload.

Uses perf on linux and dtrace otherwise.

Windows is getting [dtrace support](https://techcommunity.microsoft.com/t5/Windows-Kernel-Internals/DTrace-on-Windows/ba-p/362902), so if you try this out please let us know how it goes :D 

## Installation

```
cargo install flamegraph
```

This will make the `cargo-flamegraph` binary
available in your cargo binary directory.
On most systems this is usually something
like `~/.cargo/bin`.

## Examples

```
# defaults to profiling cargo run, which will
# also profile the cargo compilation process
# unless you've previously issued `cargo build`
cargo flamegraph

# if you'd like to profile your release build:
cargo flamegraph --release

# if you'd like to profile a specific binary:
cargo flamegraph --bin=stress2

# if you'd like to profile an arbitrary executable:
cargo flamegraph --exec="/path/to/my/binary --some-arg 5"
```

## Usage

```
USAGE:
    cargo flamegraph [FLAGS] [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -r, --release    Activate release mode
    -V, --version    Prints version information

OPTIONS:
    -b, --bin <bin>              Binary to run
    -e, --exec <exec>            Other command to run
    -f, --features <features>    Build features to enable
    -o, --output <output>        Output file, flamegraph.svg if not present
```

## Enabling perf for use by unpriviledged users

To enable perf without running as root, you may
lower the `perf_event_paranoid` value in proc
to an appropriate level for your environment.
The most permissive value is `-1` but may not
be acceptable for your security needs etc...

```bash
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```
