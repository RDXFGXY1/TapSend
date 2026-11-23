.PHONY: build test fmt android
ANDROID_GRADLE ?= ./gradlew
build:
	mkdir -p bin
	go build -buildvcs=false -o bin/tapsend-server ./cmd/tapsend-server
	cargo build --manifest-path tui/Cargo.toml --release
	cp tui/target/release/tapsend bin/tapsend.next
	mv -f bin/tapsend.next bin/tapsend

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

