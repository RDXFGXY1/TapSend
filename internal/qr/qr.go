package qr

import (
	"strings"

	qrcode "github.com/skip2/go-qrcode"
)

func PNG(value string) ([]byte, error) {
	return qrcode.Encode(value, qrcode.Medium, 512)
}

// Terminal renders two QR rows per terminal line with a white quiet zone.
func Terminal(value string) (string, error) {
	code, err := qrcode.New(value, qrcode.Medium)
	if err != nil {
		return "", err
	}
	bitmap := code.Bitmap()
	var out strings.Builder
	for y := 0; y < len(bitmap); y += 2 {
		out.WriteString("\x1b[47m\x1b[30m")
		for x := range bitmap[y] {
			top := bitmap[y][x]
			bottom := y+1 < len(bitmap) && bitmap[y+1][x]
			switch {
			case top && bottom:
				out.WriteRune('█')
			case top:
				out.WriteRune('▀')
			case bottom:
				out.WriteRune('▄')
			default:
				out.WriteByte(' ')
			}
		}
		out.WriteString("\x1b[0m\n")
	}
	return out.String(), nil
}
