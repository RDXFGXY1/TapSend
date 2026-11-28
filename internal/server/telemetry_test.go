package server

import (
	"bytes"
	"errors"
	"io"
	"net/http/httptest"
	"testing"
	"time"
)

type shortWriter struct{}

func (shortWriter) Write(p []byte) (int, error) { return 2, io.ErrShortWrite }

func TestTelemetryCountsOnlyWrittenBytes(t *testing.T) {
	progress := newTransferTelemetry("file.txt", "RX", 10)
	n, err := (telemetryWriter{shortWriter{}, progress}).Write([]byte("hello"))
	if n != 2 || !errors.Is(err, io.ErrShortWrite) || progress.done != 2 {
		t.Fatalf("n=%d err=%v done=%d", n, err, progress.done)
	}
	progress.finish("failed")
	last := progress.last
	progress.finish("saved")
	if !progress.last.Equal(last) {
		t.Fatal("finish must only report once")
	}
}

func TestTelemetryTracksRangeResponseLength(t *testing.T) {
	recorder := httptest.NewRecorder()
	progress := newTransferTelemetry("range.bin", "TX", -1)
	writer := &transferWriter{ResponseWriter: recorder, telemetry: progress, last: time.Now()}
	writer.Header().Set("Content-Length", "3")
	writer.Header().Set("Content-Range", "bytes 5-7/12")
	writer.WriteHeader(206)
	if _, err := io.Copy(writer, bytes.NewBufferString("abc")); err != nil {
		t.Fatal(err)
	}
	if progress.offset != 5 || progress.total != 3 || progress.done != 3 || writer.status != 206 {
		t.Fatalf("offset=%d total=%d done=%d status=%d", progress.offset, progress.total, progress.done, writer.status)
	}
}
