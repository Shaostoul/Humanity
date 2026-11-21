// Remembering the key on a phone (Android rung A1, 2026-11-20; android/README.md,
// docs/reference/cryptography.md "Remembering the key on a phone").
//
// Android's key store holds keys that never leave it: this class makes one AES-256-GCM key per
// remembered secret (in a separate security chip, StrongBox, when the phone has one, else in its
// secure hardware, else in software, and says which; usable only while the phone is unlocked, see
// create), and seals or opens 32 bytes with it, wiping the plaintext it was handed. The
// sealed bytes are kept by the Rust side in the app's private folder
// (crates/humanity-core/src/platform/key_wrap.rs has the file's layout). Every method is called
// from Rust through JNI (crates/humanity-core/src/platform/android_jni.rs); the signatures Rust
// uses are listed in android_java.rs, whose test reads this file.
package us.united_humanity.humanityos;

import android.os.Build;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyInfo;
import android.security.keystore.KeyProperties;
import android.util.Log;
import java.security.KeyStore;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.GCMParameterSpec;

public final class KeyStoreBridge {
    private static final String STORE = "AndroidKeyStore";
    private static final String TRANSFORM = "AES/GCM/NoPadding";
    private static final int TAG_BITS = 128;
    // Added to create's answer when the key carries the unlocked-only rule (key_wrap::UNLOCKED_ONLY,
    // which auto_unlock.rs's test checks against this line).
    private static final int UNLOCKED_ONLY = 0x100;

    private KeyStoreBridge() {}

    private static KeyStore store() throws Exception {
        KeyStore ks = KeyStore.getInstance(STORE);
        ks.load(null);
        return ks;
    }

    // A fresh key under alias, replacing any. Returns where it lives: 0 software, 1 secure
    // hardware, 2 StrongBox, 255 not said (key_wrap::Protection), plus UNLOCKED_ONLY when the key
    // carries the unlocked-only rule below (key_wrap::KeyInfo). Settings shows which this phone
    // got, since Android 8.0 and 8.1 have no such rule and a phone may refuse it.
    //
    // The alias is never the one the remembered secret is sealed by now: each remembered secret
    // has two key names that take turns (key_wrap::key_alias), and the Rust side deletes the old
    // key only once the new one has sealed and its file is written (KeyFileStore::keep), so a
    // remember that fails here, or later, leaves the old key and its file working.
    //
    // On Android 9 and later the key is made usable only while the phone is unlocked
    // (setUnlockedDeviceRequired, 2026-11-20): on a phone with a screen lock (a PIN, pattern or
    // password; a swipe-only lock never counts as locked), Android refuses to open a remembered
    // secret while the phone is locked. The app opens it at start-up, which happens after an
    // unlock, so this costs nothing. No user-authentication rule is set
    // (setUserAuthenticationRequired would ask for the phone's own lock at every use), so the
    // Quick PIN is checked by HumanityOS, not by Android; Settings says so (unlock.pin_android).
    // Tried in order: StrongBox and unlocked-only, the phone's secure hardware and unlocked-only,
    // then, should neither give a key that works (a phone without a screen lock may refuse the
    // rule), a key without it, said in the log and in Settings (unlock.locked_rule_off).
    public static int create(String alias) throws Exception {
        KeyStore ks = store();
        if (ks.containsAlias(alias)) {
            ks.deleteEntry(alias);
        }
        SecretKey key = null;
        if (Build.VERSION.SDK_INT >= 28) {
            key = tryGenerate(ks, alias, true, true);
            if (key == null) {
                key = tryGenerate(ks, alias, false, true);
            }
            if (key != null) {
                return protection(key) | UNLOCKED_ONLY;
            }
            Log.w("HumanityOS", "key store: no key usable only while the phone is unlocked could be made here; making one without that rule");
        }
        return protection(generate(alias, false, false));
    }

    // A key made with these rules that also seals, or null (and no key left under alias).
    private static SecretKey tryGenerate(KeyStore ks, String alias, boolean strongBox, boolean unlockedOnly) {
        try {
            SecretKey key = generate(alias, strongBox, unlockedOnly);
            if (works(key)) {
                return key;
            }
        } catch (Exception refused) {
            // StrongBoxUnavailableException (a ProviderException): no separate chip here; or a
            // chip or a phone that cannot make this key with these rules: the next try.
        }
        try {
            if (ks.containsAlias(alias)) {
                ks.deleteEntry(alias);
            }
        } catch (Exception ignored) {
            // The next generate replaces it anyway.
        }
        return null;
    }

    // Whether the key store lets the key seal: a rule the phone cannot honour shows up here, at the
    // first use, rather than when the person's secret is sealed.
    private static boolean works(SecretKey key) {
        try {
            Cipher c = Cipher.getInstance(TRANSFORM);
            c.init(Cipher.ENCRYPT_MODE, key);
            c.doFinal(new byte[0]);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    private static SecretKey generate(String alias, boolean strongBox, boolean unlockedOnly) throws Exception {
        KeyGenParameterSpec.Builder spec = new KeyGenParameterSpec.Builder(alias,
                KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256);
        if (Build.VERSION.SDK_INT >= 28) {
            if (strongBox) {
                spec.setIsStrongBoxBacked(true);
            }
            if (unlockedOnly) {
                spec.setUnlockedDeviceRequired(true);
            }
        }
        KeyGenerator gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, STORE);
        gen.init(spec.build());
        return gen.generateKey();
    }

    @SuppressWarnings("deprecation")
    private static int protection(SecretKey key) {
        try {
            SecretKeyFactory factory = SecretKeyFactory.getInstance(key.getAlgorithm(), STORE);
            KeyInfo info = (KeyInfo) factory.getKeySpec(key, KeyInfo.class);
            if (Build.VERSION.SDK_INT >= 31) {
                switch (info.getSecurityLevel()) {
                    case KeyProperties.SECURITY_LEVEL_STRONGBOX:
                        return 2;
                    case KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT:
                    case KeyProperties.SECURITY_LEVEL_UNKNOWN_SECURE:
                        return 1;
                    case KeyProperties.SECURITY_LEVEL_SOFTWARE:
                        return 0;
                    default:
                        return 255;
                }
            }
            return info.isInsideSecureHardware() ? 1 : 0;
        } catch (Exception e) {
            return 255;
        }
    }

    private static SecretKey key(String alias) throws Exception {
        return (SecretKey) store().getKey(alias, null);
    }

    // Seal plain with alias's key, binding aad. Returns the 12-byte nonce the key store chose,
    // then the sealed bytes with the tag last. plain (the copy JNI made of the Rust side's 32
    // bytes) is wiped once sealed, whatever happens.
    public static byte[] seal(String alias, byte[] plain, byte[] aad) throws Exception {
        try {
            SecretKey k = key(alias);
            if (k == null) {
                throw new IllegalStateException("the key store holds no key " + alias);
            }
            Cipher c = Cipher.getInstance(TRANSFORM);
            c.init(Cipher.ENCRYPT_MODE, k);
            c.updateAAD(aad);
            byte[] sealed = c.doFinal(plain);
            byte[] nonce = c.getIV();
            byte[] out = new byte[nonce.length + sealed.length];
            System.arraycopy(nonce, 0, out, 0, nonce.length);
            System.arraycopy(sealed, 0, out, nonce.length, sealed.length);
            return out;
        } finally {
            Arrays.fill(plain, (byte) 0);
        }
    }

    // Open sealed with alias's key. Null when the key store no longer holds that key; an
    // exception when the bytes or the associated data do not match it. The array returned holds
    // the plaintext: the Rust side writes zeros over it once copied (android_jni.rs, key_unseal).
    public static byte[] unseal(String alias, byte[] nonce, byte[] sealed, byte[] aad) throws Exception {
        SecretKey k = key(alias);
        if (k == null) {
            return null;
        }
        Cipher c = Cipher.getInstance(TRANSFORM);
        c.init(Cipher.DECRYPT_MODE, k, new GCMParameterSpec(TAG_BITS, nonce));
        c.updateAAD(aad);
        return c.doFinal(sealed);
    }

    // Delete alias's key; nothing when there is none.
    public static void forget(String alias) throws Exception {
        KeyStore ks = store();
        if (ks.containsAlias(alias)) {
            ks.deleteEntry(alias);
        }
    }
}
