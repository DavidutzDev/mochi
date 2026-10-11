//! gcr's secret exchange, `sx-aes-1`, which carries a secret between
//! gnome-keyring and its prompter over the session bus without it showing
//! there in the clear: Diffie-Hellman in the 1536-bit group of RFC 3526,
//! HKDF-SHA256 of the shared secret into an AES-128 key, and the secret in
//! AES-128-CBC with PKCS#7 padding and a random IV.
//!
//! Each side sends a GLib key file:
//!
//! ```text
//! [sx-aes-1]
//! public=<its public key, base64>
//! secret=<the secret, encrypted, base64>
//! iv=<base64>
//! ```
//!
//! the first without a secret. Mochi is the prompter: it begins, receives
//! the keyring's public key with the prompt, and sends the password back.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use num_bigint::BigUint;
use zeroize::Zeroizing;

const PROTOCOL: &str = "sx-aes-1";

/// The prime of RFC 3526's 1536-bit MODP group, whose generator is 2.
const PRIME: &str = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74\
020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F1437\
4FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED\
EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF05\
98DA48361C55D39A69163FA8FD24CF5F83655D23DCA3AD961C62F356208552BB\
9ED529077096966D670C354E4ABC9804F1746C08CA237327FFFFFFFFFFFFFFFF";

/// The prime's length in bytes: the shared secret is padded to it.
const PRIME_BYTES: usize = 192;

type Encrypt = cbc::Encryptor<aes::Aes128>;
type Decrypt = cbc::Decryptor<aes::Aes128>;

fn prime() -> BigUint {
    BigUint::parse_bytes(PRIME.as_bytes(), 16).expect("the prime is hexadecimal")
}

pub struct Exchange {
    private: Zeroizing<Vec<u8>>,
    public: Vec<u8>,
    key: Option<Zeroizing<[u8; 16]>>,
}

impl std::fmt::Debug for Exchange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Exchange")
            .field("derived", &self.key.is_some())
            .finish_non_exhaustive()
    }
}

impl Exchange {
    /// A new key pair: a random private key below the prime, as gcr makes
    /// one, and its public key.
    pub fn new() -> Result<Self, String> {
        let prime = prime();
        let mut private = Zeroizing::new(vec![0u8; PRIME_BYTES]);
        getrandom::fill(&mut private).map_err(|error| error.to_string())?;
        // Below the prime's highest bit, and never zero.
        private[0] &= 0x7f;
        private[PRIME_BYTES - 1] |= 1;
        let public = BigUint::from(2u8)
            .modpow(&BigUint::from_bytes_be(&private), &prime)
            .to_bytes_be();
        Ok(Self {
            private,
            public,
            key: None,
        })
    }

    /// The first message: this side's public key.
    pub fn begin(&self) -> String {
        format!("[{PROTOCOL}]\npublic={}\n", STANDARD.encode(&self.public))
    }

    /// Reads the other side's message: derives the shared key from its
    /// public key the first time, and decrypts the secret it carries, if
    /// any.
    pub fn receive(&mut self, message: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        let fields = parse(message)?;
        let field = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| STANDARD.decode(value.trim()))
                .transpose()
                .map_err(|_| format!("the exchange's {name} isn't base64"))
        };
        if self.key.is_none() {
            let peer = field("public")?.ok_or("the exchange has no public key")?;
            self.derive(&peer)?;
        }
        let Some(secret) = field("secret")? else {
            return Ok(None);
        };
        let iv = field("iv")?.ok_or("the exchange's secret has no iv")?;
        let key = self.key.as_ref().ok_or("no key")?;
        let iv: [u8; 16] = iv.try_into().map_err(|_| "the iv isn't 16 bytes")?;
        let mut buffer = Zeroizing::new(secret);
        let plain = Decrypt::new(key.as_ref().into(), &iv.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|_| "the secret doesn't decrypt")?
            .to_vec();
        Ok(Some(Zeroizing::new(plain)))
    }

    /// A message with this side's public key and, when there's one, the
    /// secret, encrypted with the shared key. `receive` must have run.
    pub fn send(&self, secret: Option<&[u8]>) -> Result<String, String> {
        let mut message = format!("[{PROTOCOL}]\npublic={}\n", STANDARD.encode(&self.public));
        if let Some(secret) = secret {
            let key = self.key.as_ref().ok_or("nothing received yet")?;
            let mut iv = [0u8; 16];
            getrandom::fill(&mut iv).map_err(|error| error.to_string())?;
            let encrypted = Encrypt::new(key.as_ref().into(), &iv.into())
                .encrypt_padded_vec_mut::<Pkcs7>(secret);
            message.push_str(&format!(
                "secret={}\niv={}\n",
                STANDARD.encode(encrypted),
                STANDARD.encode(iv)
            ));
        }
        Ok(message)
    }

    fn derive(&mut self, peer: &[u8]) -> Result<(), String> {
        let prime = prime();
        let peer = BigUint::from_bytes_be(peer);
        // A public key of 0, 1 or p - 1 would make the secret guessable.
        if peer <= BigUint::from(1u8) || peer >= &prime - 1u8 {
            return Err("the exchange's public key is out of range".into());
        }
        let shared = peer.modpow(&BigUint::from_bytes_be(&self.private), &prime);
        let bytes = Zeroizing::new(shared.to_bytes_be());
        let mut padded = Zeroizing::new(vec![0u8; PRIME_BYTES]);
        padded[PRIME_BYTES - bytes.len()..].copy_from_slice(&bytes);
        let mut key = Zeroizing::new([0u8; 16]);
        hkdf::Hkdf::<sha2::Sha256>::new(None, &padded)
            .expand(&[], key.as_mut())
            .map_err(|_| "can't derive the key")?;
        self.key = Some(key);
        Ok(())
    }
}

/// The keys of a key file's `[sx-aes-1]` group.
fn parse(message: &str) -> Result<Vec<(String, String)>, String> {
    let mut inside = false;
    let mut fields = Vec::new();
    for line in message.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(group) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            inside = group == PROTOCOL;
            continue;
        }
        if inside && let Some((key, value)) = line.split_once('=') {
            fields.push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    if fields.is_empty() {
        return Err(format!("the exchange isn't {PROTOCOL}"));
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_crosses_and_comes_out_the_same() {
        let mut prompter = Exchange::new().unwrap();
        let mut keyring = Exchange::new().unwrap();
        // The prompter begins; the keyring answers with its own key.
        assert_eq!(keyring.receive(&prompter.begin()).unwrap(), None);
        let asked = keyring.send(None).unwrap();
        assert_eq!(prompter.receive(&asked).unwrap(), None);
        // The password goes back, encrypted.
        let reply = prompter.send(Some(b"correct horse")).unwrap();
        assert!(!reply.contains("correct horse"));
        assert!(reply.starts_with("[sx-aes-1]\npublic="));
        let secret = keyring.receive(&reply).unwrap().unwrap();
        assert_eq!(secret.as_slice(), b"correct horse");
    }

    #[test]
    fn an_empty_secret_and_a_bad_message() {
        let mut one = Exchange::new().unwrap();
        let mut two = Exchange::new().unwrap();
        two.receive(&one.begin()).unwrap();
        one.receive(&two.send(None).unwrap()).unwrap();
        let empty = two.receive(&one.send(Some(b"")).unwrap()).unwrap().unwrap();
        assert!(empty.is_empty());
        assert!(
            Exchange::new()
                .unwrap()
                .receive("[other]\npublic=AA==\n")
                .is_err()
        );
        assert!(
            Exchange::new()
                .unwrap()
                .receive("[sx-aes-1]\npublic=AQ==\n")
                .is_err(),
            "a public key of 1"
        );
    }

    /// Against gcr itself: `MOCHI_GCR_SX` names a program on gcr's side,
    /// which reads the prompter's first message, answers with its own,
    /// then reads the reply and prints `SECRET:<the secret>`. Each message
    /// ends with a line `END`.
    #[test]
    #[ignore = "needs a program built against gcr, in MOCHI_GCR_SX"]
    fn gcr_reads_the_secret() {
        use std::io::{BufRead, Write};
        let program = std::env::var("MOCHI_GCR_SX").expect("MOCHI_GCR_SX");
        let mut child = std::process::Command::new(program)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut prompter = Exchange::new().unwrap();
        writeln!(input, "{}END", prompter.begin()).unwrap();
        input.flush().unwrap();
        let mut theirs = String::new();
        loop {
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            if line == "END\n" || line.is_empty() {
                break;
            }
            theirs.push_str(&line);
        }
        prompter.receive(&theirs).unwrap();
        writeln!(
            input,
            "{}END",
            prompter.send(Some(b"correct horse")).unwrap()
        )
        .unwrap();
        input.flush().unwrap();
        let mut said = String::new();
        output.read_line(&mut said).unwrap();
        assert_eq!(said.trim_end(), "SECRET:correct horse");
        drop(input);
        child.wait().unwrap();
    }

    #[test]
    fn the_prime_is_rfc_3526s() {
        let prime = prime();
        assert_eq!(prime.bits(), 1536);
        assert_eq!(prime.to_bytes_be().len(), PRIME_BYTES);
    }
}
