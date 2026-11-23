.PHONY: build test fmt android portable-linux install-linux
ANDROID_GRADLE ?= ./gradlew
RELEASE_VERSION := 1.0.0
LINUX_RELEASE := TapSend-v$(RELEASE_VERSION)-linux-$(shell uname -m)
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
	mkdir -p dist/$(LINUX_RELEASE)/bin dist/$(LINUX_RELEASE)/scripts dist/$(LINUX_RELEASE)/licenses
	cp packaging/README.release.md dist/$(LINUX_RELEASE)/README.md
	cp release.md dist/$(LINUX_RELEASE)/release.md
	cp scripts/install-linux.sh dist/$(LINUX_RELEASE)/scripts/install-linux.sh
	cp packaging/licenses/go-qrcode.LICENSE dist/$(LINUX_RELEASE)/licenses/go-qrcode.LICENSE
	cp bin/tapsend bin/tabsend bin/tapsend-tui bin/tapsend-server dist/$(LINUX_RELEASE)/bin/
	tar -czf dist/$(LINUX_RELEASE).tar.gz -C dist/$(LINUX_RELEASE) README.md release.md licenses/go-qrcode.LICENSE scripts/install-linux.sh bin/tapsend bin/tabsend bin/tapsend-tui bin/tapsend-server

install-linux: build
	bash scripts/install-linux.sh

