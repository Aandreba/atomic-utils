doc:
	cargo rustdoc --open --all-features -- --cfg docsrs

check:
	cargo check --no-default-features
	cargo check --no-default-features --features std
	cargo check --no-default-features --features alloc
	cargo check --no-default-features --features nightly
	cargo check --features futures
	cargo check --features const

miri:
	RUST_BACKTRACE=1 MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test

test:
	cargo test --no-default-features
	cargo test --no-default-features --features std
	cargo test --no-default-features --features alloc
	cargo test --no-default-features --features nightly
	cargo test --features futures
	cargo test --features const