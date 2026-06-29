/**
 * MockWalletAuthenticator — provides PBKDF2 key derivation for the bridge.
 * The bridge needs deriveMasterKey to initialize the storage master key.
 */
import { pbkdf2Sync } from "node:crypto";
export class MockWalletAuthenticator {
    /**
     * Derive a 32-byte AES-256 master key from a password and salt via PBKDF2-SHA256.
     * Uses 100,000 iterations — same parameters as the ARK SDK's production authenticator.
     */
    static deriveMasterKey(password, salt) {
        return pbkdf2Sync(password, salt, 100_000, 32, "sha256");
    }
}
//# sourceMappingURL=authenticator.js.map