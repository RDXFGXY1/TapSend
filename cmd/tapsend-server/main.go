package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

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
	defaults := config.Default()
	listen := flag.String("listen", "", "HTTPS listen address (default "+defaults.ListenAddress+")")
	data := flag.String("data-dir", "", "private TapSend data directory")
	apk := flag.String("share-apk", "", "temporarily serve one Android APK and print its download QR")
	apkHost := flag.String("apk-host", "", "private IPv4 address to use for the APK share")
	flag.Var(&files, "file", "file to share (repeatable)")
	flag.Parse()
	if *apk != "" {
		ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
		defer stop()
		if err := server.ShareAPK(ctx, *apk, *apkHost, os.Stdout); err != nil {
			log.Fatal(err)
		}
		return
	}
	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	flag.Visit(func(f *flag.Flag) {
		switch f.Name {
		case "listen":
			cfg.ListenAddress = *listen
		case "data-dir":
			cfg.DataDirectory = *data
		}
	})
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
