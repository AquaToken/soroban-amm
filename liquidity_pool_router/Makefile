default: build

all: test

test: build
	cargo test

build:
	$(MAKE) -C ../liquidity_pool || break;
	$(MAKE) -C ../liquidity_pool_stableswap || break;
	soroban contract build
	@ls -l target/wasm32-unknown-unknown/release/*.wasm
fmt:
	cargo fmt --all

clean:
	cargo clean
