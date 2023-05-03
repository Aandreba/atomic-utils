doc:
	cargo +nightly rustdoc --open --all-features -- --cfg docsrs

check:
	cargo clippy --no-default-features -- -Dwarnings
	cargo clippy --no-default-features --features std -- -Dwarnings
	cargo clippy --no-default-features --features alloc -- -Dwarnings
	cargo clippy --no-default-features --features nightly -- -Dwarnings
	cargo clippy --features alloc_api -- -Dwarnings
	cargo clippy --features futures -- -Dwarnings
	cargo clippy --features const -- -Dwarnings

miri:
	RUST_BACKTRACE=1 MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test

test: check
	cargo test --no-default-features
	cargo test --no-default-features --features std
	cargo test --no-default-features --features alloc
	cargo test --no-default-features --features nightly
	cargo test --features futures
	cargo test --features const
