package server

import (
	"errors"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

type folderDrop struct {
	sync.Mutex
	peer, directory, token string
	expires                time.Time
}

func (a *App) armFolderDrop(peer, directory string) error {
	if _, ok := a.pairing.Device(peer); !ok {
		return errors.New("choose a paired phone first")
	}
	if !filepath.IsAbs(directory) {
		return errors.New("choose an existing folder")
	}
	path, err := filepath.EvalSymlinks(directory)
	if err != nil {
		return err
	}
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return errors.New("choose an existing folder")
	}
	token, err := screenKey()
	if err != nil {
		return err
	}
	a.drop.Lock()
	defer a.drop.Unlock()
	a.drop.peer, a.drop.directory, a.drop.token = peer, path, token
	a.drop.expires = time.Now().Add(10 * time.Minute)
	return nil
}

func (a *App) folderDropFor(peer string) (string, string) {
	a.drop.Lock()
	defer a.drop.Unlock()
	if peer == "" || peer != a.drop.peer || time.Now().After(a.drop.expires) {
		return "", ""
	}
	return a.drop.directory, a.drop.token
}

func (a *App) handleFolderDrop(w http.ResponseWriter, r *http.Request) {
	dir, token := a.folderDropFor(a.requestPeer(r))
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, map[string]string{"folder": dir, "token": token})
}

func publishFolderDrop(staged, dir, name string) (string, error) {
	ext := filepath.Ext(name)
	root := strings.TrimSuffix(name, ext)
	for i := 1; i <= 1000; i++ {
		filename := name
		if i > 1 {
			filename = fmt.Sprintf("%s (%d)%s", root, i, ext)
		}
		path := filepath.Join(dir, filename)
		if err := os.Link(staged, path); err == nil {
			return path, nil
		} else if !errors.Is(err, os.ErrExist) {
			return "", err
		}
	}
	return "", errors.New("no free filename")
}
