doc:
	cargo rustdoc --open --all-features -- --cfg docsrs

miri:
	RUST_BACKTRACE=1 MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test stress