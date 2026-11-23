package files

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestAddKeepsExistingIDsAndPublishesNewFile(t *testing.T) {
	dir := t.TempDir()
	first := filepath.Join(dir, "first.txt")
	second := filepath.Join(dir, "second.txt")
	for _, name := range []string{first, second} {
		if err := os.WriteFile(name, []byte("hello"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	catalog, err := New([]string{first})
	if err != nil {
		t.Fatal(err)
	}
	initial := catalog.Public()
	if len(initial) != 1 {
		t.Fatalf("initial catalog has %d files", len(initial))
	}
	item, added, err := catalog.Add(second)
	if err != nil || !added {
		t.Fatalf("new file not added: added=%v err=%v", added, err)
	}
	if got, ok := catalog.Get(item.ID); !ok || got.Path != second {
		t.Fatalf("new item not accessible: %+v %v", got, ok)
	}
	_, added, err = catalog.Add(first)
	if err != nil || added {
		t.Fatalf("duplicate file was added again: added=%v err=%v", added, err)
	}
	all := catalog.Public()
	if len(all) != 2 || all[0].ID != initial[0].ID {
		t.Fatalf("unexpected catalog after add: %+v", all)
	}
}

func TestConcurrentReadsWhileAdding(t *testing.T) {
	dir := t.TempDir()
	catalog, err := New(nil)
	if err != nil {
		t.Fatal(err)
	}
	var readers sync.WaitGroup
	for range 8 {
		readers.Add(1)
		go func() {
			defer readers.Done()
			for range 100 {
				_ = catalog.Public()
			}
		}()
	}
	for i := 0; i < 10; i++ {
		name := filepath.Join(dir, string(rune('a'+i)))
		if err := os.WriteFile(name, []byte("x"), 0600); err != nil {
			t.Fatal(err)
		}
		if _, _, err := catalog.Add(name); err != nil {
			t.Fatal(err)
		}
	}
	readers.Wait()
	if got := len(catalog.Public()); got != 10 {
		t.Fatalf("want 10 files, got %d", got)
	}
}
