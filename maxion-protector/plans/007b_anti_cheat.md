Recommended Anti-Cheat Workflow
Instead of the server giving the client a key, use Hardware-Backed or Obfuscated Key Generation on the client:

Client-Side Generation: The game generates a unique Ed25519 key pair on the user's machine at first launch.

Registration: The client sends the Public Key to your server. The server links HardwareID_123 to PublicKey_ABC.

Signing: Every high-value action (buying an item, finishing a level) is signed by the client.

Verification: The server rejects any request that isn't signed by the key registered to that specific Hardware ID.

Strategies to Harden Ed25519 Against Tampering
To prevent the cheater from just "stealing" the key the client generated, you should use these industry-standard "Speed Bumps":

White-Box Cryptography: Don't use a standard library like libsodium out of the box. Use heavily obfuscated code so the cheater can't easily find the crypto_sign function in the binary.

Memory Protection: Store the private key in "scrambled" form in memory and only descramble it at the millisecond it is needed for signing.

Heartbeat/Sequence Numbers: Include a monotonically increasing counter or a server-sent nonce in every signed payload. This prevents "Replay Attacks," where a cheater captures a legitimate "Win Game" signature and sends it to the server 100 times.

Side-Channel Validation: Along with the signature, sign a small "blob" of game state (e.g., player coordinates + checksum of game memory). If the signature is valid but the game state is impossible, you catch the cheat.