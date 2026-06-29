/**
 * MockWalletAuthenticator — provides PBKDF2 key derivation for the bridge.
 * The bridge needs deriveMasterKey to initialize the storage master key.
 */
export declare class MockWalletAuthenticator {
    /**
     * Derive a 32-byte AES-256 master key from a password and salt via PBKDF2-SHA256.
     * Uses 100,000 iterations — same parameters as the ARK SDK's production authenticator.
     */
    static deriveMasterKey(password: string, salt: Buffer): Buffer;
}
//# sourceMappingURL=authenticator.d.ts.map