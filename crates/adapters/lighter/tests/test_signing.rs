use nautilus_lighter::signing::LighterSigner;

#[test]
fn test_real_schnorr_signing() {
    let test_private_key = "00000000000000000000000000000000000000000000000000000000000000000000000000000001";

    let signer = LighterSigner::new(
        test_private_key,
        LighterSigner::CHAIN_ID_TESTNET,
        2,
        878,
        100,
    )
    .unwrap();

    // Create a test message
    let message = [42u8; 40];

    // Sign the message
    let signature = signer.sign(&message).unwrap();

    // Verify signature length
    assert_eq!(signature.len(), 80);

    // Signature should not be all zeros
    assert_ne!(signature, [0u8; 80]);

    // Verify that different messages produce different signatures
    let message2 = [43u8; 40];
    let signature2 = signer.sign(&message2).unwrap();
    assert_ne!(signature, signature2);

    println!("✓ Real Schnorr signing works!");
    println!("  Message 1 signature (first 32 hex chars): {}", hex::encode(&signature[..16]));
    println!("  Message 2 signature (first 32 hex chars): {}", hex::encode(&signature2[..16]));
}

#[test]
fn test_signature_verification() {
    use goldilocks_crypto::ScalarField;

    // Generate a private key as ScalarField
    let private_key_scalar = ScalarField::new([1, 0, 0, 0, 0]);
    let private_key_bytes = private_key_scalar.to_bytes_le();

    // Create signer with this private key
    let private_key_hex = hex::encode(private_key_bytes);
    let signer = LighterSigner::new(
        &private_key_hex,
        LighterSigner::CHAIN_ID_TESTNET,
        2,
        878,
        100,
    )
    .unwrap();

    // Sign a message
    let message = [7u8; 40];
    let signature = signer.sign(&message).unwrap();

    // NOTE: The goldilocks-crypto verify_signature function currently expects
    // the private key as the third parameter (not the public key).
    let is_valid = LighterSigner::verify(&signature, &message, &private_key_bytes).unwrap();
    assert!(is_valid, "Signature should be valid");

    // Verify that wrong message fails
    let wrong_message = [8u8; 40];
    let is_valid_wrong = LighterSigner::verify(&signature, &wrong_message, &private_key_bytes)
        .unwrap_or(false);
    assert!(!is_valid_wrong, "Signature should be invalid for wrong message");

    println!("✓ Signature verification works!");
}

#[test]
fn test_deterministic_signing() {
    let test_private_key = "00000000000000000000000000000000000000000000000000000000000000000000000000000001";

    let signer = LighterSigner::new(
        test_private_key,
        LighterSigner::CHAIN_ID_TESTNET,
        2,
        878,
        100,
    )
    .unwrap();

    let message = [99u8; 40];
    let sig1 = signer.sign(&message).unwrap();
    let sig2 = signer.sign(&message).unwrap();

    // Should be identical since nonce is the same
    assert_eq!(sig1, sig2, "Signatures should be deterministic with same nonce");

    println!("✓ Deterministic signing works!");
    println!("  Signature (hex): {}", hex::encode(sig1));
}
