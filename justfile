default: dev

alias r := run
alias rr := runrelease
alias t := test
alias i := info
alias f := fmt

dev:
    cargo qemu --clog="DEBUG" --flog="NONE" --slog="TRACE"

info:
    cargo qemu --clog="INFO" --flog="NONE" --slog="TRACE"

trace:
    cargo qemu --clog="TRACE" --flog="NONE" --slog="TRACE"

dbg:
    cargo qemu --clog="DEBUG" --flog="NONE" --slog="TRACE" --debug

run:
    cargo qemu --clog="NONE" --flog="NONE" --slog="NONE"

runrelease:
    cargo qemu --clog="NONE" --flog="NONE" --slog="NONE" --release

test:
    cargo ktest --clog="NONE" --flog="NONE" --slog="NONE"

gdb:
    riscv64-unknown-elf-gdb -ex 'file target/riscv64imac-unknown-none-elf/kernel' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

fmt:
    cargo fmt
    taplo format