//! Standalone test for Schnorr signing implementation.
//!
//! This example demonstrates that the goldilocks-crypto integration works correctly.
//!
//! Run with: cargo run --example test_signing_standalone --package nautilus-lighter

use goldilocks_crypto::schnorr::{sign_with_nonce, verify_signature};
use goldilocks_crypto::ScalarField;

fn main() {
    println!("=== Standalone Schnorr Signing Test ===\n");

    // Test 1: Basic signing
    println!("Test 1: Basic signing with goldilocks-crypto");
    let private_key_scalar = ScalarField::new([1, 0, 0, 0, 0]);
    let private_key_bytes = private_key_scalar.to_bytes_le();

    let message = [42u8; 40];

    // Convert nonce to bytes
    let nonce = 100u64;
    let mut nonce_bytes = [0u8; 40];
    nonce_bytes[..8].copy_from_slice(&nonce.to_le_bytes());

    // Sign the message
    match sign_with_nonce(&private_key_bytes, &message, &nonce_bytes) {
        Ok(signature) => {
            println!("  ✓ Signing successful!");
            println!("  Signature length: {} bytes", signature.len());
            println!("  Signature (first 32 hex): {}", hex::encode(&signature[..16]));

            // Test 2: Verify the signature
            println!("\nTest 2: Verify signature");
            match verify_signature(&signature, &message, &private_key_bytes) {
                Ok(is_valid) => {
                    if is_valid {
                        println!("  ✓ Signature verification PASSED!");
                    } else {
                        println!("  ✗ Signature verification FAILED!");
                    }
                }
                Err(e) => {
                    println!("  ✗ Verification error: {}", e);
                }
            }

            // Test 3: Verify with wrong message should fail
            println!("\nTest 3: Verify with wrong message (should fail)");
            let wrong_message = [43u8; 40];
            match verify_signature(&signature, &wrong_message, &private_key_bytes) {
                Ok(is_valid) => {
                    if !is_valid {
                        println!("  ✓ Correctly rejected wrong message!");
                    } else {
                        println!("  ✗ Incorrectly accepted wrong message!");
                    }
                }
                Err(e) => {
                    println!("  ✓ Correctly rejected with error: {}", e);
                }
            }

            // Test 4: Deterministic signing
            println!("\nTest 4: Deterministic signing (same nonce = same signature)");
            let sig2 = sign_with_nonce(&private_key_bytes, &message, &nonce_bytes).unwrap();
            if signature == sig2 {
                println!("  ✓ Signatures are deterministic!");
            } else {
                println!("  ✗ Signatures differ!");
            }

            // Test 5: Different messages produce different signatures
            println!("\nTest 5: Different messages produce different signatures");
            let message2 = [99u8; 40];
            let sig3 = sign_with_nonce(&private_key_bytes, &message2, &nonce_bytes).unwrap();
            if signature != sig3 {
                println!("  ✓ Different messages produce different signatures!");
                println!("  Sig1 (first 32 hex): {}", hex::encode(&signature[..16]));
                println!("  Sig3 (first 32 hex): {}", hex::encode(&sig3[..16]));
            } else {
                println!("  ✗ Signatures are the same!");
            }
        }
        Err(e) => {
            println!("  ✗ Signing failed: {}", e);
        }
    }

    println!("\n=== All Tests Complete ===");
}
