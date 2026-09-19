WASM_PACK ?= wasm-pack

.PHONY: wasm
wasm:
	$(WASM_PACK) build --release --target web --out-dir pkg
