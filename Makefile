.PHONY: build run test clean

build:
	$(MAKE) -C os build

run: build
	bash tools/run-qemu.sh

test: build
	python3 tests/boot.py

clean:
	cd os && cargo clean
	cd user && cargo clean
	cd easy-fs-fuse && cargo clean
