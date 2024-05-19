all:
	rm .cargo -rf
	cp cargo-submit .cargo -r
	cp res/rustsbi-qemu.bin sbi-qemu
	cargo xbuild --clog="NONE" --flog="NONE" --slog="NONE" --release