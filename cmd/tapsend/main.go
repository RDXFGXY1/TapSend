package main

import (
	"bufio"
	"context"
	"flag"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"

	"tapsend/internal/config"
	"tapsend/internal/devices"
	"tapsend/internal/relay"
	"tapsend/internal/sendstatus"
	"tapsend/internal/server"
	"tapsend/internal/updates"
)

var releaseVersion = "1.0.2"
var releaseRepository = "RDXFGXY1/TapSend"

func main() { os.Exit(run(os.Args[1:], os.Stdin, os.Stdout, os.Stderr)) }

func run(args []string, input io.Reader, output, errorsOut io.Writer) int {
	if len(args) == 0 {
		return launchTUI(input, output, errorsOut)
	}
	switch args[0] {
	case "version", "--version", "-V":
		fmt.Fprintln(output, "TapSend "+releaseVersion)
		return 0
	case "update-check":
		if len(args) != 1 {
			return fail(errorsOut, "update-check takes no arguments")
		}
		latest, err := updates.Check(context.Background(), releaseRepository, releaseVersion)
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		if latest == nil {
			fmt.Fprintln(output, "current\t"+releaseVersion)
		} else {
			fmt.Fprintln(output, "available\t"+releaseVersion+"\t"+latest.Tag+"\t"+latest.URL)
		}
		return 0
	case "help", "-h", "--help":
		fmt.Fprint(output, usage)
		return 0
	case "devices":
		if len(args) != 1 {
			return fail(errorsOut, "devices takes no arguments")
		}
		cfg, err := config.Load()
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		store, err := devices.Open(cfg.DataDirectory)
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		items := store.List()
		if len(items) == 0 {
			fmt.Fprintln(output, "No paired devices. Open tabsend to pair a phone.")
			return 0
		}
		for _, device := range items {
			fmt.Fprintf(output, "%s\t%s\n", device.Name, device.ID)
		}
		return 0
	case "send":
		return send(args[1:], input, output, errorsOut)
	case "receive":
		return receive(args[1:], output, errorsOut)
	case "status":
		if len(args) != 1 {
			return fail(errorsOut, "status takes no arguments")
		}
		cfg, err := config.Load()
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		state, err := sendstatus.Read(cfg.DataDirectory)
		if os.IsNotExist(err) {
			fmt.Fprintln(output, "No targeted send has started yet.")
			return 0
		}
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		fmt.Fprintf(output, "%s → %s\nFiles: %s\nUpdated: %s\n", state.State, state.Device, strings.Join(state.Files, ", "), state.UpdatedAt.Local().Format("2006-01-02 15:04:05"))
		return 0
	case "settings":
		return settings(args[1:], output, errorsOut)
	case "relay":
		return runRelay(args[1:], input, output, errorsOut)
	default:
		return fail(errorsOut, fmt.Sprintf("unknown command %q\n%s", args[0], usage))
	}
}

func runRelay(args []string, input io.Reader, out, errorsOut io.Writer) int {
	cfg, err := config.Load()
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	result := ""
	switch {
	case len(args) == 2 && args[0] == "create":
		result, err = relay.Create(cfg.DataDirectory, args[1])
	case len(args) == 1 && args[0] == "connect":
		var pass []byte
		pass, err = io.ReadAll(io.LimitReader(input, 8193))
		if err == nil {
			err = relay.Connect(cfg.DataDirectory, string(pass))
			result = "Relay paired · send a focused file"
		}
	case len(args) == 2 && args[0] == "send":
		result, err = relay.Send(cfg.DataDirectory, args[1])
	case len(args) == 1 && args[0] == "status":
		result, err = relay.Status(cfg.DataDirectory)
	default:
		return fail(errorsOut, "relay commands: create SHARED_FOLDER, connect (pass from stdin), send FILE, status")
	}
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	fmt.Fprintln(out, result)
	return 0
}

const usage = `TapSend — share files with paired phones

  tabsend                                      Open the terminal interface
  tabsend devices                              List paired phones
  tabsend send -d "My Phone" -p "photo.jpg"    Send to one paired phone
  tabsend send -p "photo.jpg" -d "My Phone"    Flags can be reordered
  tabsend send "My Phone" "photo.jpg"          Positional shorthand
  tabsend receive                              Accept phone uploads without sharing PC files
  tabsend status                               Show the latest send status
  tabsend settings                             Show saved settings
  tabsend settings set --inbox DIR --start-folder DIR --show-hidden=true
  tabsend settings set --listen :8443 --pairing-host HOST --direct-joins 2
  tabsend settings defaults                     Preview default settings
  tabsend settings reset                        Restore default settings
  tabsend --version                             Show the release version

Use -p/--path more than once for multiple files. Quote names and paths with spaces.
`

func fail(w io.Writer, message string) int { fmt.Fprintln(w, "tabsend:", message); return 2 }

func launchTUI(input io.Reader, output, errorsOut io.Writer) int {
	binary := "tapsend-tui"
	if own, err := os.Executable(); err == nil {
		candidate := filepath.Join(filepath.Dir(own), "tapsend-tui"+exeSuffix())
		if _, err := os.Stat(candidate); err == nil {
			binary = candidate
		}
	}
	command := exec.Command(binary)
	command.Stdin, command.Stdout, command.Stderr = input, output, errorsOut
	if err := command.Run(); err != nil {
		return fail(errorsOut, err.Error())
	}
	return 0
}

func exeSuffix() string {
	if os.PathSeparator == '\\' {
		return ".exe"
	}
	return ""
}

type sendArgs struct {
	device string
	paths  []string
	help   bool
}

func parseSend(args []string) (sendArgs, error) {
	var result sendArgs
	var positional []string
	flagsEnded := false
	for index := 0; index < len(args); index++ {
		arg := args[index]
		if !flagsEnded && arg == "--" {
			flagsEnded = true
			continue
		}
		if !flagsEnded && (arg == "--help" || arg == "-h") {
			result.help = true
			return result, nil
		}
		name, value, found := strings.Cut(arg, "=")
		if !flagsEnded && (name == "-d" || name == "--device" || name == "-p" || name == "--path") {
			if !found {
				index++
				if index >= len(args) {
					return result, fmt.Errorf("%s needs a value", name)
				}
				value = args[index]
			}
			if value == "" {
				return result, fmt.Errorf("%s needs a value", name)
			}
			if name == "-d" || name == "--device" {
				if result.device != "" {
					return result, fmt.Errorf("device was specified more than once")
				}
				result.device = value
			} else {
				result.paths = append(result.paths, value)
			}
			continue
		}
		if !flagsEnded && strings.HasPrefix(arg, "-") {
			return result, fmt.Errorf("unknown option %s", arg)
		}
		positional = append(positional, arg)
	}
	if result.device == "" && len(positional) > 0 {
		result.device = positional[0]
		positional = positional[1:]
	}
	if len(positional) > 0 && len(result.paths) > 0 {
		return result, fmt.Errorf("use -p for each additional path")
	}
	result.paths = append(result.paths, positional...)
	return result, nil
}

func send(args []string, input io.Reader, output, errorsOut io.Writer) int {
	parsed, err := parseSend(args)
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	if parsed.help {
		fmt.Fprint(output, usage)
		return 0
	}
	cfg, err := config.Load()
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	store, err := devices.Open(cfg.DataDirectory)
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	items := store.List()
	interactive := false
	if file, ok := input.(*os.File); ok {
		if info, err := file.Stat(); err == nil {
			interactive = info.Mode()&os.ModeCharDevice != 0
		}
	}
	reader := bufio.NewReader(input)
	if parsed.device == "" {
		if !interactive {
			return fail(errorsOut, "device required: use -d/--device")
		}
		for _, device := range items {
			fmt.Fprintf(output, "%s  %s\n", device.Name, device.ID)
		}
		fmt.Fprint(output, "Device name or ID: ")
		parsed.device, _ = reader.ReadString('\n')
		parsed.device = strings.TrimSpace(parsed.device)
	}
	if len(parsed.paths) == 0 {
		if !interactive {
			return fail(errorsOut, "path required: use -p/--path")
		}
		fmt.Fprint(output, "File path: ")
		path, _ := reader.ReadString('\n')
		parsed.paths = []string{strings.TrimSpace(path)}
	}
	var target devices.Device
	var found bool
	for _, device := range items {
		if device.ID == parsed.device {
			target, found = device, true
			break
		}
		if device.Name == parsed.device {
			if found {
				return fail(errorsOut, "device name is ambiguous; use its ID from tabsend devices")
			}
			target, found = device, true
		}
	}
	if !found {
		return fail(errorsOut, fmt.Sprintf("device %q is not paired; run tabsend devices", parsed.device))
	}
	for _, path := range parsed.paths {
		absolute, err := filepath.Abs(path)
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		info, err := os.Stat(absolute)
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		if !info.Mode().IsRegular() {
			return fail(errorsOut, fmt.Sprintf("not a regular file: %s", absolute))
		}
		cfg.SharedFiles = append(cfg.SharedFiles, absolute)
	}
	app, err := server.New(cfg)
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	if err := app.SelectRecipient(target.ID, target.Name); err != nil {
		return fail(errorsOut, err.Error())
	}
	fmt.Fprintf(output, "Offering %d file(s) to %s. Open TapSend on that phone.\n", len(cfg.SharedFiles), target.Name)
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()
	running, cancel := context.WithCancel(ctx)
	defer cancel()
	errors := make(chan error, 1)
	go func() { errors <- app.RunContext(running) }()
	select {
	case <-app.Done():
		fmt.Fprintln(output, "Saved on phone.")
		cancel()
		if err := <-errors; err != nil {
			return fail(errorsOut, err.Error())
		}
		return 0
	case err := <-errors:
		if err != nil {
			app.SetSendStatus("failed")
			return fail(errorsOut, err.Error())
		}
		app.SetSendStatus("cancelled")
		return 1
	case <-ctx.Done():
		app.SetSendStatus("cancelled")
		cancel()
		<-errors
		return 1
	}
}

func receive(args []string, output, errorsOut io.Writer) int {
	if len(args) == 1 && (args[0] == "-h" || args[0] == "--help") {
		fmt.Fprintln(output, "Usage: tabsend receive\nStart TapSend without sharing computer files, then send files from a paired phone.")
		return 0
	}
	if len(args) != 0 {
		return fail(errorsOut, "receive takes no arguments")
	}
	cfg, err := config.Load()
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	app, err := server.New(cfg)
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	_, port, err := net.SplitHostPort(cfg.ListenAddress)
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	fmt.Fprintf(output, "Ready to receive into %s\nPortal Mode: https://localhost:%s/portal.html\nPairing link: %s\nManual code: %s\nSecurity key: %s\nCA certificate: %s\nPress Ctrl+C to stop.\n", cfg.InboxDirectory, port, app.PairingURL(), app.ManualCode(), app.ManualKey(), app.CAPath())
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()
	if err := app.RunContext(ctx); err != nil {
		return fail(errorsOut, err.Error())
	}
	return 0
}

func settings(args []string, output, errorsOut io.Writer) int {
	if len(args) == 1 && args[0] == "state" {
		path, err := config.SettingsPath()
		if err != nil {
			return fail(errorsOut, err.Error())
		}
		if _, err = os.Stat(path); os.IsNotExist(err) {
			fmt.Fprintln(output, "missing")
		} else if err != nil {
			return fail(errorsOut, err.Error())
		} else if _, err = config.Load(); err != nil {
			return fail(errorsOut, err.Error())
		} else {
			fmt.Fprintln(output, "configured")
		}
		return 0
	}
	if len(args) == 1 && args[0] == "defaults" {
		printSettings(output, config.Default())
		return 0
	}
	if len(args) == 1 && args[0] == "reset" {
		if err := config.Save(config.Default()); err != nil {
			return fail(errorsOut, err.Error())
		}
		fmt.Fprintln(output, "Settings reset to defaults. Restart an active share to apply them.")
		return 0
	}
	cfg, err := config.Load()
	if err != nil {
		return fail(errorsOut, err.Error())
	}
	if len(args) == 0 || args[0] == "show" {
		if len(args) > 1 {
			return fail(errorsOut, "settings show takes no arguments")
		}
		printSettings(output, cfg)
		return 0
	}
	onboarding := args[0] == "setup"
	if args[0] != "set" && !onboarding {
		return fail(errorsOut, "use tabsend settings or tabsend settings set")
	}
	set := flag.NewFlagSet("settings set", flag.ContinueOnError)
	set.SetOutput(errorsOut)
	inbox := set.String("inbox", cfg.InboxDirectory, "computer inbox directory")
	listen := set.String("listen", cfg.ListenAddress, "listen address")
	start := set.String("start-folder", cfg.StartDirectory, "starting folder for TUI")
	data := set.String("data-dir", cfg.DataDirectory, "private device and certificate storage")
	hidden := set.Bool("show-hidden", cfg.ShowHidden, "show hidden files by default")
	advertise := set.String("pairing-host", cfg.AdvertiseHost, "host/IP included in pairing links")
	direct := set.Int("direct-joins", cfg.DirectJoinLimit, "number of devices allowed before approval")
	uploadLimit := set.Int("upload-limit-gib", cfg.UploadLimitGiB, "maximum size of a phone upload in GiB")
	portal := set.Bool("portal-enabled", cfg.PortalEnabled, "allow Portal Mode on this computer")
	if err := set.Parse(args[1:]); err != nil {
		return 2
	}
	if len(set.Args()) > 0 {
		return fail(errorsOut, "unexpected settings arguments")
	}
	cfg.InboxDirectory, cfg.ListenAddress = *inbox, *listen
	cfg.StartDirectory, cfg.ShowHidden = *start, *hidden
	cfg.AdvertiseHost, cfg.DirectJoinLimit = *advertise, *direct
	cfg.DataDirectory, cfg.UploadLimitGiB, cfg.PortalEnabled = *data, *uploadLimit, *portal
	if onboarding {
		if err := config.PrepareStorage(cfg); err != nil {
			return fail(errorsOut, err.Error())
		}
	}
	if err := config.Save(cfg); err != nil {
		return fail(errorsOut, err.Error())
	}
	fmt.Fprintln(output, "Settings saved. Restart an active share for connection changes to take effect.")
	return 0
}

func printSettings(output io.Writer, cfg config.Config) {
	fmt.Fprintf(output, "inbox\t%s\nstart_folder\t%s\nshow_hidden\t%t\nlisten\t%s\npairing_host\t%s\ndirect_joins\t%d\ndata_dir\t%s\nupload_limit_gib\t%d\nportal_enabled\t%t\n",
		cfg.InboxDirectory, cfg.StartDirectory, cfg.ShowHidden, cfg.ListenAddress, cfg.AdvertiseHost, cfg.DirectJoinLimit, cfg.DataDirectory, cfg.UploadLimitGiB, cfg.PortalEnabled)
}
