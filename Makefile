.PHONY: coverage

doc:
	cargo +nightly rustdoc --open --all-features -- --cfg docsrs

check:
	rustup component add clippy
	cargo clippy --no-default-features -- -Dwarnings
	cargo clippy --no-default-features --features std -- -Dwarnings
	cargo clippy --no-default-features --features alloc -- -Dwarnings
	cargo +nightly clippy --no-default-features --features nightly -- -Dwarnings
	cargo +nightly clippy --features alloc_api -- -Dwarnings
	cargo clippy --features futures -- -Dwarnings
	cargo clippy --features const -- -Dwarnings

miri:
	rustup component add miri
	RUST_BACKTRACE=1 MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test

test: check
	cargo test --no-default-features
	cargo test --no-default-features --features std
	cargo test --no-default-features --features alloc
	cargo +nightly test --no-default-features --features nightly
	cargo +nightly test --features alloc_api
	cargo test --features futures
	cargo test --features const

coverage:
	rustup component add llvm-tools-preview
	cargo install grcov
	rm -r coverage/*
	mkdir tmp
	LLVM_PROFILE_FILE="coverage/tmp/%p-%m.profraw" RUSTFLAGS="-Cinstrument-coverage" cargo test
	grcov ./coverage -s . --binary-path ./target/debug/ -t html,markdown --branch --ignore-not-existing -o ./coverage
