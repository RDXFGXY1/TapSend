package server

import (
	"io"
	"log"
	"sync/atomic"
	"time"
)

var transferSequence atomic.Uint64

// Each HTTP request has its own identity, including parallel range requests.
type transferTelemetry struct {
	id                    uint64
	name, direction       string
	file, peer            string
	offset                int64
	total, done, previous int64
	last                  time.Time
	finished              bool
}

func newTransferTelemetry(name, direction string, total int64) *transferTelemetry {
	return startTransferTelemetry(name, direction, total, "", "")
}

func startTransferTelemetry(name, direction string, total int64, file, peer string) *transferTelemetry {
	t := &transferTelemetry{id: transferSequence.Add(1), name: name, direction: direction, total: total,
		file: file, peer: peer, offset: -1, last: time.Now()}
	t.report("active")
	return t
}
func (t *transferTelemetry) add(n int) {
	t.done += int64(n)
	if time.Since(t.last) >= time.Second {
		t.report("active")
	}
}
func (t *transferTelemetry) report(state string) {
	elapsed := time.Since(t.last).Seconds()
	speed := float64(0)
	if elapsed > 0 {
		speed = float64(t.done-t.previous) / elapsed
	}
	log.Printf("TUI_JOB id=%d dir=%s done=%d total=%d speed=%.0f state=%s name=%x file=%s peer=%x offset=%d",
		t.id, t.direction, t.done, t.total, speed, state, []byte(t.name), t.file, t.peer, t.offset)
	t.previous, t.last = t.done, time.Now()
}
func (t *transferTelemetry) finish(state string) {
	if t.finished {
		return
	}
	t.finished = true
	t.report(state)
}

type telemetryWriter struct {
	destination io.Writer
	telemetry   *transferTelemetry
}

func (w telemetryWriter) Write(p []byte) (int, error) {
	n, err := w.destination.Write(p)
	w.telemetry.add(n)
	return n, err
}
