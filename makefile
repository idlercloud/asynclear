all:
	rm .cargo -r
	cp cargo-submit .cargo
	cp res/rustsbi-qemu.bin sbi-qemu
	cargo xbuild --clog="NONE" --flog="NONE" --slog="NONE" --release