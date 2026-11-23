package com.tapsend.android;

import android.net.Uri;
import android.util.Base64;

import java.util.regex.Pattern;
import java.util.Locale;

final class PairingLink {
    private static final Pattern CODE = Pattern.compile("[A-Za-z0-9_-]{22}");
    private static final Pattern MANUAL_CODE = Pattern.compile("[0-9]{8}");
    private static final String MANUAL_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    final String base;
    final String code;
    final byte[] certificatePin;

    private PairingLink(String base, String code, byte[] certificatePin) {
        this.base = base;
        this.code = code;
        this.certificatePin = certificatePin;
    }

    static PairingLink parse(String text) {
        return parse(text, false);
    }

    private static PairingLink parse(String text, boolean manual) {
        Uri uri = Uri.parse(text.trim());
        String host = uri.getHost();
        int port = uri.getPort();
        String fragment = uri.getFragment();
        if (!"https".equalsIgnoreCase(uri.getScheme()) || host == null || !privateIPv4(host)
                || port < 1 || port > 65535 || fragment == null) {
            throw new IllegalArgumentException("Scan a TapSend QR from a computer on this local network.");
        }
        String[] parts = fragment.split("~", -1);
        if (parts.length != 2 || !(manual ? MANUAL_CODE : CODE).matcher(parts[0]).matches()) {
            throw new IllegalArgumentException(manual ? "Enter the 8-digit code shown in the TUI." :
                    "This QR is missing the app security code. Update TapSend on the computer and press r for a fresh QR.");
        }
        byte[] pin;
        try {
            pin = Base64.decode(parts[1], Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
        } catch (IllegalArgumentException error) {
            throw new IllegalArgumentException("Invalid certificate fingerprint in QR.", error);
        }
        if (pin.length != (manual ? 8 : 16)) {
            throw new IllegalArgumentException("Invalid certificate fingerprint in QR.");
        }
        return new PairingLink("https://" + host + ":" + port, parts[0], pin);
    }

    static PairingLink fromManual(String address, String code, String key) {
        String base = address.trim();
        if (!base.startsWith("https://")) base = "https://" + base;
        String digits = code.replaceAll("[\\s-]", "");
        if (!MANUAL_CODE.matcher(digits).matches())
            throw new IllegalArgumentException("Enter the 8-digit pairing code shown in the TUI.");
        byte[] pin = decodeManualKey(key);
        String encoded = Base64.encodeToString(pin, Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
        return parse(base + "/#" + digits + "~" + encoded, true);
    }

    private static byte[] decodeManualKey(String raw) {
        String key = raw.replaceAll("[\\s-]", "").toUpperCase(Locale.US)
                .replace('O', '0').replace('I', '1').replace('L', '1');
        if (key.length() != 13)
            throw new IllegalArgumentException("Enter the 13-character security key shown in the TUI.");
        byte[] result = new byte[8];
        for (int i = 0; i < key.length(); i++) {
            int value = MANUAL_ALPHABET.indexOf(key.charAt(i));
            if (value < 0) throw new IllegalArgumentException("The security key contains an invalid character.");
            for (int bit = 0; bit < 5; bit++) {
                int position = i * 5 + bit;
                int digit = (value >> (4 - bit)) & 1;
                if (position == 64) {
                    if (digit != 0) throw new IllegalArgumentException("The security key is invalid.");
                } else if (digit != 0) {
                    result[position / 8] |= (byte) (1 << (7 - position % 8));
                }
            }
        }
        return result;
    }

    private static boolean privateIPv4(String host) {
        String[] parts = host.split("\\.");
        if (parts.length != 4) return false;
        int[] octets = new int[4];
        try {
            for (int i = 0; i < 4; i++) {
                if (parts[i].isEmpty() || parts[i].length() > 3) return false;
                octets[i] = Integer.parseInt(parts[i]);
                if (octets[i] < 0 || octets[i] > 255) return false;
            }
        } catch (NumberFormatException error) {
            return false;
        }
        return octets[0] == 10 || octets[0] == 192 && octets[1] == 168
                || octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31;
    }
}
