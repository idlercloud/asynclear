default: list

alias d := dev
alias r := run
alias rr := run_release
alias kt := ktest
alias ct := cargo_test
alias l := lint

list:
    just --list

# 在 QEMU 中调试运行内核
dev $KERNEL_CLOG="INFO" $KERNEL_FLOG="NONE" $KERNEL_SLOG="TRACE":
    @echo KERNEL_CLOG="$KERNEL_CLOG" KERNEL_FLOG="$KERNEL_FLOG" KERNEL_SLOG="$KERNEL_SLOG"
    cargo qemu

# 在 QEMU 中调试运行内核，并等待调试器连接
dbg $KERNEL_CLOG="DEBUG" $KERNEL_FLOG="NONE" $KERNEL_SLOG="TRACE":
    @echo KERNEL_CLOG="$KERNEL_CLOG" KERNEL_FLOG="$KERNEL_FLOG" KERNEL_SLOG="$KERNEL_SLOG"
    cargo qemu --debug

run: (dev "NONE" "NONE" "NONE")

run_release:
    cargo qemu --release

lint:
    cargo lint

ktest $KERNEL_CLOG="NONE" $KERNEL_FLOG="NONE" $KERNEL_SLOG="NONE":
    @echo KERNEL_CLOG="$KERNEL_CLOG" KERNEL_FLOG="$KERNEL_FLOG" KERNEL_SLOG="$KERNEL_SLOG"
    cargo ktest

cargo_test:
    cargo test -p fat32 --no-default-features --features std

gdb:
    riscv64-unknown-elf-gdb -ex 'file target/riscv64imac-unknown-none-elf/kernel' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

print_home_folder:
    echo "HOME is: '${HOME}'"
