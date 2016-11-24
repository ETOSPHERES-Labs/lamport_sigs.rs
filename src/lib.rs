//! *lamport* implements one-time hash-based signatures using the Lamport signature scheme.

extern crate crypto;
extern crate rand;

use rand::OsRng;
use rand::Rng;
use crypto::digest::Digest;

pub type LamportSignatureData = Vec<Vec<u8>>;

/// A one-time signing public key
#[derive(Clone)]
pub struct PublicKey<T: Digest + Clone> {
    zero_values: Vec<Vec<u8>>,
    one_values: Vec<Vec<u8>>,
    digest: T,
}

/// A one-time signing private key
#[derive(Clone)]
pub struct PrivateKey<T: Digest + Clone> {
    // For a n bits hash function: (n * n/8 bytes) for zero_values and one_values
    zero_values: Vec<Vec<u8>>,
    one_values: Vec<Vec<u8>>,
    digest: T,
    used: bool,
}

impl<T: Digest + Clone> From<PublicKey<T>> for Vec<u8> {
    fn from(original: PublicKey<T>) -> Vec<u8> {
        original.to_bytes()
    }
}

impl<T: Digest + Clone> PublicKey<T> {
    pub fn values(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        (self.zero_values.clone(), self.one_values.clone())
    }

    pub fn from_vec(vec: Vec<u8>, digest: T) -> Option<PublicKey<T>> {
        let size = vec.len();
        let hash_output_size = digest.output_bytes();

        let mut zero_values_merged = vec;
        let one_values_merged = zero_values_merged.split_off(size / 2);

        let mut zero_values = Vec::new();
        for i in (0..zero_values_merged.len()).filter(|x| x % hash_output_size == 0) {
            // indexes for heads
            let mut sub_vec = Vec::new();
            for j in 0..hash_output_size {
                sub_vec.push(zero_values_merged[i + j]);
            }

            zero_values.push(sub_vec);
        }

        let mut one_values = Vec::new();
        for i in (0..one_values_merged.len()).filter(|x| x % hash_output_size == 0) {
            // indexes for heads
            let mut sub_vec = Vec::new();
            for j in 0..hash_output_size {
                sub_vec.push(one_values_merged[i + j]);
            }

            one_values.push(sub_vec);
        }

        Some(PublicKey {
            zero_values: zero_values,
            one_values: one_values,
            digest: digest,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.zero_values.iter().chain(self.one_values.iter()).fold(Vec::new(), |mut acc, i| {
            acc.append(&mut i.clone());
            acc
        })
    }

    /// Verifies that the signature of the data is correctly signed with the given key
    pub fn verify_signature(&self, signature: &LamportSignatureData, data: &[u8]) -> bool {
        let mut digest = self.digest.clone();
        digest.input(data);
        let mut data_hash = vec![0 as u8; digest.output_bytes()];
        digest.result(data_hash.as_mut_slice());
        digest.reset();

        for (i, byte) in data_hash.iter().enumerate() {
            for j in 0..8 {
                let offset = i * 8 + j;
                if (byte & (1 << j)) > 0 {
                    digest.input(signature[offset].as_slice());
                    let mut hashed_value = vec![0 as u8; digest.output_bytes()];
                    digest.result(hashed_value.as_mut_slice());
                    digest.reset();
                    if hashed_value != self.one_values[offset] {
                        return false;
                    }
                } else {
                    digest.input(signature[offset].as_slice());
                    let mut hashed_value = vec![0 as u8; digest.output_bytes()];
                    digest.result(hashed_value.as_mut_slice());
                    digest.reset();
                    if hashed_value != self.zero_values[offset] {
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl<T: Digest + Clone> PrivateKey<T> {
    /// Generates a new random one-time signing key. This method can panic if OS RNG fails
    pub fn new(digest: T) -> PrivateKey<T> {
        let generate_bit_hash_values = |hasher: &T| -> Vec<Vec<u8>> {
            let mut rng = match OsRng::new() {
                Ok(g) => g,
                Err(e) => panic!("Failed to obtain OS RNG: {}", e),
            };
            let buffer_byte = vec![0 as u8; hasher.output_bytes()];
            let mut buffer = vec![buffer_byte; hasher.output_bits()];

            for hash in &mut buffer {
                rng.fill_bytes(hash)
            }

            println!("{:?}", buffer);

            buffer
        };

        let zero_values = generate_bit_hash_values(&digest);
        let one_values = generate_bit_hash_values(&digest);

        PrivateKey {
            zero_values: zero_values,
            one_values: one_values,
            digest: digest,
            used: false,
        }
    }

    /// Returns the public key associated with this private key
    pub fn public_key(&self) -> PublicKey<T> {
        let mut digest = self.digest.clone();

        let hash_values = |x: &Vec<Vec<u8>>, hash_func: &mut Digest| -> Vec<Vec<u8>> {
            let buffer_byte = vec![0 as u8; hash_func.output_bytes()];
            let mut buffer = vec![buffer_byte; hash_func.output_bits()];

            for i in 0..hash_func.output_bits() {
                hash_func.input(x[i].as_slice());
                hash_func.result(buffer[i].as_mut_slice());
                hash_func.reset();
            }

            buffer
        };

        let hashed_zero_values = hash_values(&self.zero_values, &mut digest);
        let hashed_one_values = hash_values(&self.one_values, &mut digest);

        PublicKey {
            zero_values: hashed_zero_values,
            one_values: hashed_one_values,
            digest: digest,
        }
    }

    /// Signs the data with the private key and returns the result if successful.
    /// If unsuccesful, an explanation string is returned
    pub fn sign(&mut self, data: &[u8]) -> Result<LamportSignatureData, &'static str> {
        if self.used {
            return Err("Attempting to sign more than once.");
        }
        self.digest.input(data);
        let mut data_hash = vec![0 as u8; self.digest.output_bytes()];
        self.digest.result(data_hash.as_mut_slice());
        self.digest.reset();

        let signature_len = data_hash.len() * 8;
        let mut signature = Vec::with_capacity(signature_len);

        for (i, byte) in data_hash.iter().enumerate() {
            for j in 0..8 {
                let offset = i * 8 + j;
                if (byte & (1 << j)) > 0 {
                    // Bit is 1
                    signature.push(self.one_values[offset].clone());
                } else {
                    // Bit is 0
                    signature.push(self.zero_values[offset].clone());
                }
            }
        }
        self.used = true;
        Ok(signature)
    }
}

impl<T: Digest + Clone> Drop for PrivateKey<T> {
    fn drop(&mut self) {
        let zeroize_vector = |vector: &mut Vec<Vec<u8>>| {
            for v2 in vector.iter_mut() {
                for byte in v2.iter_mut() {
                    *byte = 0;
                }
            }
        };

        zeroize_vector(&mut self.zero_values);
        zeroize_vector(&mut self.one_values);
    }
}

impl<T: Digest + Clone> PartialEq for PrivateKey<T> {
    // ⚠️ This is not a constant-time implementation
    fn eq(&self, other: &PrivateKey<T>) -> bool {
        if self.one_values.len() != other.one_values.len() {
            return false;
        }
        if self.zero_values.len() != other.zero_values.len() {
            return false;
        }

        for i in 0..self.zero_values.len() {
            if self.zero_values[i] != other.zero_values[i] ||
               self.one_values[i] != other.one_values[i] {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
pub mod tests;
