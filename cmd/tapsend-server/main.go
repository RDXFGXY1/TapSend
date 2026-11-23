package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"

	"tapsend/internal/config"
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

	if len(files) == 0 {
		fmt.Fprintln(os.Stderr, "at least one --file is required")
		os.Exit(2)
	}

	app, err := server.New(cfg)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("TapSend pairing URL: %s", app.PairingURL())
	log.Printf("TapSend manual details: %s %s", app.ManualCode(), app.ManualKey())
	log.Printf("Install this CA certificate on the phone once: %s", app.CAPath())
	go app.Control(os.Stdin)
	log.Fatal(app.Run())
}
