.PHONY: build test fmt android portable-linux install-linux
ANDROID_GRADLE ?= ./gradlew
build:
	mkdir -p bin
	go build -buildvcs=false -o bin/tapsend ./cmd/tapsend
	cp bin/tapsend bin/tabsend
	go build -buildvcs=false -o bin/tapsend-server ./cmd/tapsend-server
	cargo build --manifest-path tui/Cargo.toml --release
	cp tui/target/release/tapsend bin/tapsend-tui.next
	mv -f bin/tapsend-tui.next bin/tapsend-tui

test:
	go test ./...
	cargo test --manifest-path tui/Cargo.toml

fmt:
	gofmt -w cmd internal
	cargo fmt --manifest-path tui/Cargo.toml

android:
	cd android && $(ANDROID_GRADLE) assembleDebug
	mkdir -p bin
	cp android/app/build/outputs/apk/debug/app-debug.apk bin/tapsend-android-debug.apk

portable-linux: build
	mkdir -p dist/tapsend-linux-$(shell uname -m)/bin dist/tapsend-linux-$(shell uname -m)/scripts
	cp README.md dist/tapsend-linux-$(shell uname -m)/README.md
	cp scripts/install-linux.sh dist/tapsend-linux-$(shell uname -m)/scripts/install-linux.sh
	cp bin/tapsend bin/tabsend bin/tapsend-tui bin/tapsend-server dist/tapsend-linux-$(shell uname -m)/bin/
	tar -czf dist/tapsend-linux-$(shell uname -m).tar.gz -C dist/tapsend-linux-$(shell uname -m) README.md scripts/install-linux.sh bin/tapsend bin/tabsend bin/tapsend-tui bin/tapsend-server

install-linux: build
	bash scripts/install-linux.sh

