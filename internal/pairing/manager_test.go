package pairing

import (
	"crypto/ed25519"
	"crypto/rand"
	"fmt"
	"strings"
	"testing"

	"tapsend/internal/devices"
)

func TestRefreshingPairingCodeInvalidatesOldCode(t *testing.T) {
	store, err := devices.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	oldCode := manager.NewPairingCode()
	newCode := manager.NewPairingCode()
	if oldCode == newCode {
		t.Fatal("refresh did not change pairing code")
	}
	key := make([]byte, ed25519.PublicKeySize)
	if err := manager.Pair(oldCode, "old", "Old phone", key); err == nil || !strings.Contains(err.Error(), "invalid") {
		t.Fatalf("old pairing code was accepted: %v", err)
	}
	if err := manager.Pair(newCode, "new", "New phone", key); err != nil {
		t.Fatalf("new pairing code was rejected: %v", err)
	}
}

func TestManualCodeHasGuessLimitAndDoesNotWeakenQR(t *testing.T) {
	store, err := devices.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	qrCode := manager.NewPairingCode()
	manual := manager.ManualCode()
	if len(manual) != 8 {
		t.Fatalf("manual code length: %q", manual)
	}
	key := make([]byte, ed25519.PublicKeySize)
	if err := manager.Pair(manual, "phone-one", "Phone one", key); err != nil {
		t.Fatal(err)
	}
	for attempt := 0; attempt < 10; attempt++ {
		guess := fmt.Sprintf("%08d", attempt)
		if guess == manual {
			guess = "12345678"
			if guess == manual {
				guess = "87654321"
			}
		}
		if err := manager.Pair(guess, "guess", "Guess", key); err == nil {
			t.Fatal("wrong manual code accepted")
		}
	}
	if err := manager.Pair(manual, "phone-two", "Phone two", key); err == nil {
		t.Fatal("manual code was not locked after ten wrong guesses")
	}
	if err := manager.Pair(qrCode, "phone-three", "Phone three", key); err != nil {
		t.Fatalf("QR code stopped working after manual guesses: %v", err)
	}
}

func TestThirdDeviceWaitsForSenderApproval(t *testing.T) {
	store, err := devices.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	code := manager.NewPairingCode()
	var tokens []string
	for _, id := range []string{"phone-aaaa", "phone-bbbb", "phone-cccc", "phone-dddd"} {
		public, private, err := ed25519.GenerateKey(rand.Reader)
		if err != nil {
			t.Fatal(err)
		}
		if err := manager.Pair(code, id, id, public); err != nil {
			t.Fatal(err)
		}
		challenge, err := manager.Challenge(id)
		if err != nil {
			t.Fatal(err)
		}
		token, status, err := manager.Verify(id, ed25519.Sign(private, []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		if got := manager.Access(token); got != status {
			t.Fatalf("%s: access %s, join %s", id, got, status)
		}
		tokens = append(tokens, token)
	}
	if manager.Access(tokens[0]) != JoinAllowed || manager.Access(tokens[1]) != JoinAllowed {
		t.Fatal("first two phones should join")
	}
	if manager.Access(tokens[2]) != JoinPending || manager.Access(tokens[3]) != JoinPending {
		t.Fatal("third and fourth phones must wait")
	}
	if len(manager.Pending()) != 2 {
		t.Fatal("missing join requests")
	}
	if err := manager.Approve("phone-c"); err != nil {
		t.Fatal(err)
	}
	if manager.Access(tokens[2]) != JoinAllowed {
		t.Fatal("approval did not unlock the existing session")
	}
	if err := manager.Deny("phone-d"); err != nil {
		t.Fatal(err)
	}
	if manager.Access(tokens[3]) != JoinDenied {
		t.Fatal("denial did not block the session")
	}
}
