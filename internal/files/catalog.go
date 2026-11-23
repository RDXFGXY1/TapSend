package files

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"sync"
)

type Item struct {
	ID, Name, Path string
	Size           int64
}
type PublicItem struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Size int64  `json:"size"`
}
type Catalog struct {
	mu    sync.RWMutex
	items map[string]Item
	paths map[string]string
}

func New(paths []string) (*Catalog, error) {
	c := &Catalog{items: make(map[string]Item), paths: make(map[string]string)}
	for _, path := range paths {
		if _, _, err := c.Add(path); err != nil {
			return nil, err
		}
	}
	return c, nil
}

// Add publishes a regular file to already paired receivers without restarting HTTPS.
func (c *Catalog) Add(path string) (Item, bool, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return Item{}, false, err
	}
	info, err := os.Stat(abs)
	if err != nil {
		return Item{}, false, err
	}
	if !info.Mode().IsRegular() {
		return Item{}, false, fmt.Errorf("not a regular file: %s", abs)
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	if id, ok := c.paths[abs]; ok {
		return c.items[id], false, nil
	}
	id := randomID()
	item := Item{ID: id, Name: filepath.Base(abs), Path: abs, Size: info.Size()}
	c.items[id] = item
	c.paths[abs] = id
	return item, true, nil
}

func (c *Catalog) Public() []PublicItem {
	c.mu.RLock()
	defer c.mu.RUnlock()
	items := make([]PublicItem, 0, len(c.items))
	for _, item := range c.items {
		items = append(items, PublicItem{item.ID, item.Name, item.Size})
	}
	sort.Slice(items, func(i, j int) bool {
		if items[i].Name == items[j].Name {
			return items[i].ID < items[j].ID
		}
		return items[i].Name < items[j].Name
	})
	return items
}
func (c *Catalog) Get(id string) (Item, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	item, ok := c.items[id]
	return item, ok
}
func randomID() string {
	data := make([]byte, 16)
	_, _ = rand.Read(data)
	return hex.EncodeToString(data)
}
