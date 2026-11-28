// Package relay carries encrypted deliveries through a folder on a separate Windows PC.
package relay

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const passPrefix = "TSR1-"
const maxFile = 8 << 30
const blockSize = 256 << 10

type route struct {
	Folder string
	Key    []byte
}

type delivery struct {
	ID, Name string
}

type fileHeader struct {
	Name string
	Size int64
}

func Create(data, folder string) (string, error) {
	if !filepath.IsAbs(folder) {
		return "", errors.New("use the full path of a shared folder on the relay computer")
	}
	if _, err := os.Stat(filepath.Join(data, "relay-receiver.json")); !os.IsNotExist(err) {
		if err != nil {
			return "", err
		}
		r, err := loadRoute(data, "receiver")
		if err != nil || filepath.Dir(r.Folder) != filepath.Clean(folder) {
			return "", errors.New("this computer already has a relay route")
		}
		return r.pass()
	}
	id, err := newID()
	if err != nil {
		return "", err
	}
	key := make([]byte, 32)
	if _, err := rand.Read(key); err != nil {
		return "", err
	}
	r := route{Folder: filepath.Join(folder, "TapSend-"+id), Key: key}
	if err := os.Mkdir(r.Folder, 0700); err != nil {
		return "", fmt.Errorf("create lane on the relay computer: %w", err)
	}
	if err = save(data, "relay-receiver.json", r); err != nil {
		_ = os.Remove(r.Folder)
		return "", err
	}
	return r.pass()
}

func (r route) pass() (string, error) {
	b, err := json.Marshal(r)
	if err != nil {
		return "", err
	}
	return passPrefix + base64.RawURLEncoding.EncodeToString(b), nil
}

func Connect(data, pass string) error {
	encoded, ok := strings.CutPrefix(strings.TrimSpace(pass), passPrefix)
	if !ok || len(encoded) > 8192 {
		return errors.New("copy a TapSend relay pass from the destination computer")
	}
	payload, err := base64.RawURLEncoding.DecodeString(encoded)
	if err != nil {
		return errors.New("relay pass is invalid")
	}
	var r route
	if json.Unmarshal(payload, &r) != nil || !r.valid() {
		return errors.New("relay pass is invalid")
	}
	info, err := os.Stat(r.Folder)
	if err != nil || !info.IsDir() {
		return errors.New("the relay folder is unavailable from this computer")
	}
	return save(data, "relay-sender.json", r)
}

func Send(data, path string) (string, error) {
	r, err := loadRoute(data, "sender")
	if err != nil {
		return "", err
	}
	in, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer in.Close()
	info, err := in.Stat()
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() || info.Size() > maxFile || !validName(info.Name()) {
		return "", errors.New("select a supported file up to 8 GiB")
	}
	id, err := newID()
	if err != nil {
		return "", err
	}
	box, err := r.box(id)
	if err != nil {
		return "", err
	}
	header, err := json.Marshal(fileHeader{info.Name(), info.Size()})
	if err != nil {
		return "", err
	}
	err = stage(r.Folder, id+".relay", func(out *os.File) error {
		if err := box.write(out, header); err != nil {
			return err
		}
		input := make([]byte, blockSize)
		for remaining := info.Size(); remaining > 0; {
			chunk := input[:min(int64(len(input)), remaining)]
			if _, err := io.ReadFull(in, chunk); err != nil {
				return err
			}
			if err := box.write(out, chunk); err != nil {
				return err
			}
			remaining -= int64(len(chunk))
		}
		if err := checkEnd(in); err != nil {
			return errors.New("the selected file changed while it was being read")
		}
		return nil
	})
	if err != nil {
		return "", err
	}
	if err := save(data, "relay-delivery.json", delivery{id, info.Name()}); err != nil {
		return "", fmt.Errorf("file staged on the relay as %s but local status could not be saved: %w", id, err)
	}
	return "This PC → relay (holding) → destination PC (" + info.Name() + ")", nil
}

func Status(data string) (string, error) {
	r, err := loadRoute(data, "sender")
	if err != nil {
		return "", err
	}
	var d delivery
	if err = load(data, "relay-delivery.json", &d); os.IsNotExist(err) {
		return "Relay paired · focus a file and send it", nil
	}
	if err != nil || !validID(d.ID) {
		return "", errors.New("could not read the last relay delivery")
	}
	f, err := os.Open(filepath.Join(r.Folder, d.ID+".saved"))
	state := "relay (holding) → destination PC"
	if err == nil {
		defer f.Close()
		receipt, readErr := io.ReadAll(io.LimitReader(f, sha256.Size+1))
		if readErr != nil || !hmac.Equal(receipt, r.receipt(d.ID)) {
			return "", errors.New("the relay returned an invalid save confirmation")
		}
		state = "relay → destination PC (SAVED)"
	} else if !os.IsNotExist(err) {
		return "", err
	} else if _, err := os.Stat(filepath.Join(r.Folder, d.ID+".relay")); err != nil {
		return "Relay unavailable or delivery missing · " + d.Name, nil
	}
	return "This PC → " + state + " · " + d.Name, nil
}

func Collect(data, inbox string, saved func(string)) {
	r, err := loadRoute(data, "receiver")
	if err != nil {
		return
	}
	files, err := os.ReadDir(r.Folder)
	if err != nil {
		return
	}
	for _, file := range files {
		id, isDelivery := strings.CutSuffix(file.Name(), ".relay")
		if !isDelivery || !validID(id) || !file.Type().IsRegular() {
			continue
		}
		if name, err := r.collect(id, inbox); err == nil {
			if name != "" {
				saved(name)
			}
			_ = os.Remove(filepath.Join(r.Folder, file.Name()))
		}
	}
}

func (r route) collect(id, inbox string) (string, error) {
	in, err := os.Open(filepath.Join(r.Folder, id+".relay"))
	if err != nil {
		return "", err
	}
	defer in.Close()
	box, err := r.box(id)
	if err != nil {
		return "", err
	}
	header, err := box.read(in, 4096)
	if err != nil {
		return "", err
	}
	var metadata fileHeader
	if json.Unmarshal(header, &metadata) != nil || !validName(metadata.Name) ||
		metadata.Size < 0 || metadata.Size > maxFile {
		return "", errors.New("invalid encrypted file metadata")
	}
	folder := filepath.Join(inbox, "Relay")
	if err := os.MkdirAll(folder, 0700); err != nil {
		return "", err
	}
	filename := "relay-" + id + "-" + metadata.Name
	existing, err := os.Open(filepath.Join(folder, filename))
	if err == nil {
		defer existing.Close()
		hash := sha256.New()
		if _, err := io.Copy(hash, io.LimitReader(existing, metadata.Size+1)); err != nil {
			return "", err
		}
		actual := sha256.New()
		if err := readPayload(in, actual, box, metadata.Size); err != nil {
			return "", err
		}
		if !hmac.Equal(hash.Sum(nil), actual.Sum(nil)) {
			return "", errors.New("a different file already exists at the relay destination")
		}
		filename = "" // The previous save completed before its receipt reached the relay.
	} else if !os.IsNotExist(err) {
		return "", err
	} else if err := stage(folder, filename, func(out *os.File) error {
		return readPayload(in, out, box, metadata.Size)
	}); err != nil {
		return "", err
	}
	err = stage(r.Folder, id+".saved", func(out *os.File) error {
		_, err := out.Write(r.receipt(id))
		return err
	})
	return filename, err
}

func readPayload(input io.Reader, output io.Writer, box *frames, size int64) error {
	for remaining := size; remaining > 0; {
		want := int(min(blockSize, remaining))
		chunk, err := box.read(input, want)
		if err != nil || len(chunk) != want {
			return errors.New("encrypted relay file is incomplete or invalid")
		}
		if _, err := output.Write(chunk); err != nil {
			return err
		}
		remaining -= int64(want)
	}
	return checkEnd(input)
}

func checkEnd(r io.Reader) error {
	var byteAtEnd [1]byte
	if _, err := io.ReadFull(r, byteAtEnd[:]); err != io.EOF {
		return errors.New("unexpected extra relay data")
	}
	return nil
}

type frames struct {
	cipher.AEAD
	next uint64
}

func (r route) box(id string) (*frames, error) {
	block, err := aes.NewCipher(r.sign("payload", id))
	if err != nil {
		return nil, err
	}
	box, err := cipher.NewGCM(block)
	return &frames{AEAD: box}, err
}

func (box *frames) nonce() []byte {
	var nonce [12]byte
	binary.BigEndian.PutUint64(nonce[4:], box.next)
	box.next++
	return nonce[:]
}

func (box *frames) write(out io.Writer, data []byte) error {
	sealed := box.Seal(nil, box.nonce(), data, nil)
	if err := binary.Write(out, binary.BigEndian, uint32(len(sealed))); err != nil {
		return err
	}
	_, err := out.Write(sealed)
	return err
}

func (box *frames) read(in io.Reader, limit int) ([]byte, error) {
	var length uint32
	if err := binary.Read(in, binary.BigEndian, &length); err != nil {
		return nil, err
	}
	if length < uint32(box.Overhead()) || length > uint32(limit+box.Overhead()) {
		return nil, errors.New("invalid relay frame size")
	}
	b := make([]byte, length)
	if _, err := io.ReadFull(in, b); err != nil {
		return nil, err
	}
	return box.Open(b[:0], box.nonce(), b, nil)
}

func (r route) sign(purpose, id string) []byte {
	mac := hmac.New(sha256.New, r.Key)
	_, _ = io.WriteString(mac, purpose+"\x00"+id)
	return mac.Sum(nil)
}

func (r route) receipt(id string) []byte { return r.sign("saved", id) }

func newID() (string, error) {
	id := make([]byte, 16)
	_, err := rand.Read(id)
	return hex.EncodeToString(id), err
}

func validID(id string) bool {
	payload, err := hex.DecodeString(id)
	return err == nil && len(payload) == 16
}

func validName(name string) bool {
	return name != "" && len(name) <= 190 && filepath.IsLocal(name) &&
		!strings.ContainsAny(name, "/\\:\x00") && strings.TrimRight(name, ". ") == name
}

func (r route) valid() bool {
	id, ok := strings.CutPrefix(filepath.Base(r.Folder), "TapSend-")
	return len(r.Key) == 32 && filepath.IsAbs(r.Folder) && ok && validID(id)
}

func loadRoute(data, role string) (route, error) {
	var r route
	err := load(data, "relay-"+role+".json", &r)
	if os.IsNotExist(err) {
		err = errors.New("no relay route set up")
	}
	if err == nil && !r.valid() {
		err = errors.New("saved relay route is invalid")
	}
	return r, err
}

func load(dir, name string, state any) error {
	f, err := os.Open(filepath.Join(dir, name))
	if err != nil {
		return err
	}
	defer f.Close()
	return json.NewDecoder(io.LimitReader(f, 16384)).Decode(state)
}

func save(dir, name string, state any) error {
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	return stage(dir, name, func(out *os.File) error { return json.NewEncoder(out).Encode(state) })
}

func stage(dir, name string, write func(*os.File) error) error {
	out, err := os.CreateTemp(dir, ".tapsend-relay-*")
	if err != nil {
		return err
	}
	defer os.Remove(out.Name())
	if err = write(out); err == nil {
		err = out.Sync()
	}
	if closeErr := out.Close(); err == nil {
		err = closeErr
	}
	if err != nil {
		return err
	}
	return os.Rename(out.Name(), filepath.Join(dir, name))
}
