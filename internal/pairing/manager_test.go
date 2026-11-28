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

func TestRenameAndForgetPairedDevices(t *testing.T) {
	dir := t.TempDir()
	store, err := devices.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	code := manager.NewPairingCode()
	tokens := make(map[string]string)
	for _, id := range []string{"one", "two"} {
		public, private, err := ed25519.GenerateKey(nil)
		if err != nil {
			t.Fatal(err)
		}
		if err := manager.Pair(code, id, " ", public); err != nil {
			t.Fatal(err)
		}
		device, _ := store.Get(id)
		if device.Name != "Unnamed device" {
			t.Fatalf("missing fallback name: %q", device.Name)
		}
		challenge, err := manager.Challenge(id)
		if err != nil {
			t.Fatal(err)
		}
		token, _, err := manager.Verify(id, ed25519.Sign(private, []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		tokens[id] = token
	}
	if err := manager.Rename("one", " My Pixel "); err != nil {
		t.Fatal(err)
	}
	reopened, err := devices.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	if device, ok := reopened.Get("one"); !ok || device.Name != "My Pixel" ||
		manager.Access(tokens["one"]) != JoinAllowed {
		t.Fatal("rename changed the device identity or did not persist")
	}
	if err := manager.Revoke("one"); err != nil {
		t.Fatal(err)
	}
	if manager.Access(tokens["one"]) != JoinDenied || manager.Access(tokens["two"]) != JoinAllowed {
		t.Fatal("forgetting one device changed the wrong session")
	}
	if err := manager.RevokeAll(); err != nil {
		t.Fatal(err)
	}
	reopened, err = devices.Open(dir)
	if err != nil || len(reopened.List()) != 0 || manager.Access(tokens["two"]) != JoinDenied {
		t.Fatal("cleared devices were restored or their sessions stayed authorized")
	}
	if err := manager.Pair(code, "new", "New phone", make([]byte, ed25519.PublicKeySize)); err == nil {
		t.Fatal("clear paired devices kept the previous invitation valid")
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

func TestConfiguredDirectJoinLimit(t *testing.T) {
	store, err := devices.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	manager.SetDirectJoinLimit(1)
	code := manager.NewPairingCode()
	for index, id := range []string{"first", "second"} {
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
		_, status, err := manager.Verify(id, ed25519.Sign(private, []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		want := JoinAllowed
		if index == 1 {
			want = JoinPending
		}
		if status != want {
			t.Fatalf("%s: got %s, want %s", id, status, want)
		}
	}
}

func TestSelectedRecipientBlocksOtherPairedDevices(t *testing.T) {
	store, err := devices.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	manager := New(store)
	code := manager.NewPairingCode()
	tokens := make(map[string]string)
	for _, id := range []string{"target", "other"} {
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
		token, _, err := manager.Verify(id, ed25519.Sign(private, []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		tokens[id] = token
	}
	if err := manager.Select("target"); err != nil {
		t.Fatal(err)
	}
	if got := manager.Access(tokens["target"]); got != JoinAllowed {
		t.Fatalf("target access: %s", got)
	}
	if got := manager.Access(tokens["other"]); got != JoinDenied {
		t.Fatalf("other access: %s", got)
	}
}
