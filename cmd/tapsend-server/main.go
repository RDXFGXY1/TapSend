package main

import (
	"flag"
	"fmt"
	"log"
	"net"
	"os"
	"path/filepath"

	"tapsend/internal/config"
	"tapsend/internal/qr"
	"tapsend/internal/server"
)

type fileList []string

func (f *fileList) String() string { return fmt.Sprint([]string(*f)) }
func (f *fileList) Set(value string) error {
	abs, err := filepath.Abs(value)
	if err != nil {
		return err
	}
	*f = append(*f, abs)
	return nil
}

func main() {
	if len(os.Args) == 3 && os.Args[1] == "--qr-utf8" {
		picture, err := qr.Terminal(os.Args[2])
		if err != nil {
			log.Fatal(err)
		}
		fmt.Print(picture)
		return
	}
	var files fileList
	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	flag.StringVar(&cfg.ListenAddress, "listen", cfg.ListenAddress, "HTTPS listen address")
	flag.StringVar(&cfg.DataDirectory, "data-dir", cfg.DataDirectory, "private TapSend data directory")
	flag.Var(&files, "file", "file to share (repeatable)")
	flag.Parse()
	cfg.SharedFiles = files

	app, err := server.New(cfg)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("TapSend pairing URL: %s", app.PairingURL())
	if _, port, err := net.SplitHostPort(cfg.ListenAddress); err == nil {
		log.Printf("TapSend Portal Mode: https://localhost:%s/portal.html", port)
	}
	log.Printf("TapSend manual details: %s %s", app.ManualCode(), app.ManualKey())
	log.Printf("Install this CA certificate on the phone once: %s", app.CAPath())
	go app.Control(os.Stdin)
	log.Fatal(app.Run())
}
