#[macro_use]
extern crate honggfuzz;
extern crate orion;
extern crate ring;
pub mod utils;

use orion::hazardous::constants::SHA512_OUTSIZE;
use orion::hazardous::kdf::{hkdf, pbkdf2};
use utils::{make_seeded_rng, ChaChaRng, RngCore};

fn fuzz_hkdf(fuzzer_input: &[u8], seeded_rng: &mut ChaChaRng) {
    let mut ikm = vec![0u8; fuzzer_input.len() / 2];
    seeded_rng.fill_bytes(&mut ikm);

    let mut salt = vec![0u8; fuzzer_input.len() / 4];
    seeded_rng.fill_bytes(&mut salt);

    let mut orion_okm: Vec<u8> =
        if (fuzzer_input.len() / 2) > (255 * SHA512_OUTSIZE) || (fuzzer_input.len() / 2) < 1 {
            vec![0u8; 256]
        } else {
            vec![0u8; fuzzer_input.len() / 2]
        };

    let mut other_okm = orion_okm.clone();

    // Empty info will be the same as None.
    let info: Vec<u8> = if fuzzer_input.is_empty() {
        vec![0u8; 0]
    } else {
        vec![0u8; fuzzer_input[0] as usize]
    };

    // orion
    let orion_prk = hkdf::extract(&salt, &ikm).unwrap();
    hkdf::expand(&orion_prk, Some(&info), &mut orion_okm).unwrap();

    // ring
    let other_salt = ring::hmac::SigningKey::new(&ring::digest::SHA512, &salt);
    let other_prk = ring::hkdf::extract(&other_salt, &ikm);
    ring::hkdf::expand(&other_prk, &info, &mut other_okm[..]);

    // We cannot compare PRKs because ring's SigningKey does not offer
    // access to internal bytes.
    assert_eq!(orion_okm, other_okm);
    // Test extract-then-expand combination
    hkdf::derive_key(&salt, &ikm, Some(&info), &mut orion_okm).unwrap();
    ring::hkdf::extract_and_expand(&other_salt, &ikm, &info, &mut other_okm);
    assert_eq!(orion_okm, other_okm);
}

fn fuzz_pbkdf2(fuzzer_input: &[u8], seeded_rng: &mut ChaChaRng) {
    let mut password = vec![0u8; fuzzer_input.len() / 2];
    seeded_rng.fill_bytes(&mut password);

    let mut salt = vec![0u8; fuzzer_input.len() / 4];
    seeded_rng.fill_bytes(&mut salt);

    // Cast to u16 so we don't have too many blocks to process.
    let dk_length = seeded_rng.next_u32() as u16;

    let mut orion_dk: Vec<u8> = if dk_length == 0 {
        vec![0u8; 64]
    } else {
        vec![0u8; dk_length as usize]
    };

    let mut other_dk = orion_dk.clone();
    // Cast to u16 so we don't have too many iterations.
    let mut iterations = seeded_rng.next_u32() as u16;
    if iterations == 0 {
        iterations = 1;
    }

    // orion
    let orion_password = pbkdf2::Password::from_slice(&password).unwrap();
    pbkdf2::derive_key(&orion_password, &salt, iterations as usize, &mut orion_dk).unwrap();

    // ring
    ring::pbkdf2::derive(
        &ring::digest::SHA512,
        std::num::NonZeroU32::new(u32::from(iterations)).unwrap(),
        &salt,
        &password,
        &mut other_dk,
    );

    assert_eq!(orion_dk, other_dk);
}

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            // Seed the RNG
            let mut seeded_rng = make_seeded_rng(data);

            // Test `orion::hazardous::kdf::hkdf`
            fuzz_hkdf(data, &mut seeded_rng);
            // Test `orion::hazardous::kdf::pbkdf2`
            fuzz_pbkdf2(data, &mut seeded_rng);
        });
    }
}
