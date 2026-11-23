package server

import (
	"context"
	"testing"
)

func TestStopCancelsActiveTransfersAndBlocksNewOnes(t *testing.T) {
	var state sessionState
	ctx, done, ok := state.begin(context.Background())
	if !ok {
		t.Fatal("initial transfer blocked")
	}
	defer done()
	state.stop()
	select {
	case <-ctx.Done():
	default:
		t.Fatal("active transfer was not cancelled")
	}
	_, _, ok = state.begin(context.Background())
	if ok {
		t.Fatal("new transfer accepted after stop")
	}
	state.resume()
	_, finish, ok := state.begin(context.Background())
	if !ok {
		t.Fatal("transfer did not resume")
	}
	finish()
}
