package server

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"tapsend/internal/files"
)

func TestControlAddsFileToLiveCatalog(t *testing.T) {
	path := filepath.Join(t.TempDir(), "new file.txt")
	if err := os.WriteFile(path, []byte("new data"), 0600); err != nil {
		t.Fatal(err)
	}
	catalog, err := files.New(nil)
	if err != nil {
		t.Fatal(err)
	}
	command, err := json.Marshal(map[string]string{"Action": "add", "Name": path})
	if err != nil {
		t.Fatal(err)
	}
	(&App{catalog: catalog}).Control(strings.NewReader(string(command) + "\n"))
	shared := catalog.Public()
	if len(shared) != 1 || shared[0].Name != "new file.txt" || shared[0].Size != 8 {
		t.Fatalf("file was not published: %+v", shared)
	}
}
