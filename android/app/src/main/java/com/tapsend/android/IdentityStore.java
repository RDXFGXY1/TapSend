package com.tapsend.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;

import java.security.KeyFactory;
import java.security.KeyStore;
import java.security.PrivateKey;
import java.security.SecureRandom;
import java.security.spec.PKCS8EncodedKeySpec;
import java.util.Arrays;

import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

final class IdentityStore {
    static final class Identity {
        final String base;
        final byte[] pin;
        final String deviceId;
        final PrivateKey privateKey;

        Identity(String base, byte[] pin, String deviceId, PrivateKey privateKey) {
            this.base = base;
            this.pin = pin;
            this.deviceId = deviceId;
            this.privateKey = privateKey;
        }
    }

    private static final String ALIAS = "tapsend-device-secret";
    private final SharedPreferences prefs;

    IdentityStore(Context context) {
        prefs = context.getSharedPreferences("paired-device", Context.MODE_PRIVATE);
    }

    void save(Identity identity) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, key());
        byte[] secret = identity.privateKey.getEncoded();
        byte[] encrypted = cipher.doFinal(secret);
        Arrays.fill(secret, (byte) 0);
        if (!prefs.edit()
                .putString("base", identity.base)
                .putString("pin", b64(identity.pin))
                .putString("id", identity.deviceId)
                .putString("iv", b64(cipher.getIV()))
                .putString("private", b64(encrypted))
                .commit()) {
            throw new IllegalStateException("Could not save phone identity.");
        }
    }

    Identity load() throws Exception {
        String base = prefs.getString("base", null);
        if (base == null) return null;
        byte[] iv = decode(prefs.getString("iv", ""));
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, key(), new GCMParameterSpec(128, iv));
        byte[] secret = cipher.doFinal(decode(prefs.getString("private", "")));
        try {
            PrivateKey privateKey = KeyFactory.getInstance("Ed25519")
                    .generatePrivate(new PKCS8EncodedKeySpec(secret));
            return new Identity(base, decode(prefs.getString("pin", "")),
                    prefs.getString("id", ""), privateKey);
        } finally {
            Arrays.fill(secret, (byte) 0);
        }
    }

    void clear() {
        prefs.edit().clear().apply();
    }

    private SecretKey key() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        if (store.containsAlias(ALIAS)) return ((KeyStore.SecretKeyEntry) store.getEntry(ALIAS, null)).getSecretKey();
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        generator.init(new KeyGenParameterSpec.Builder(ALIAS,
                KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(), new SecureRandom());
        return generator.generateKey();
    }

    private static String b64(byte[] bytes) {
        return Base64.encodeToString(bytes, Base64.NO_WRAP);
    }

    private static byte[] decode(String value) {
        return Base64.decode(value, Base64.DEFAULT);
    }
}
