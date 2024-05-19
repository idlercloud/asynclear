all:
	rm .cargo -r
	cp cargo-submit .cargo
	cargo xbuild --clog="NONE" --flog="NONE" --slog="NONE" --release