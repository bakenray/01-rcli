use crate::{get_reader, TextSignFormat, process_genpass};
use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::fs;
use std::io::Read;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey, SigningKey};
use rand::rngs::SysRng;
use rand::rand_core::UnwrapErr;
use std::path::Path;

pub trait TextSign {
    // Sign the data from the  reader and return the signature
    // &dyn 动态分派，更加灵活，不会产生额外代码，缺点是，代码效率相对低
    fn sign(&self, reader: &mut dyn Read) -> Result<Vec<u8>>;            
}

pub trait TextVerify {
    // Verify the data from the reader with the signature
    // 静态分派, 产生的代码体积大一点，业务代码，其实不用纠结。
    fn verify(&self, reader: impl Read, sig: &[u8]) -> Result<bool>; 
}

pub trait KeyLoader {
    fn load(path: impl AsRef<Path>) -> Result<Self>
    where
        Self: Sized; //not str /[u8]
}

pub trait KeyGenerator {
    fn generate() -> Result<Vec<Vec<u8>>>;
}

pub struct Blake3 {
    key: [u8; 32],
}

pub struct Ed25519Signer {
    key: SigningKey,
}

pub struct Ed25519Verifier {
    key: VerifyingKey
}

impl TextSign for Blake3 {
    fn sign(&self, reader: &mut dyn Read) -> Result<Vec<u8>> {
        //TODO: improve perf by reading in chunks
        let mut buf: Vec<u8> = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(blake3::keyed_hash(&self.key, &buf).as_bytes().to_vec())
    }
}

impl TextVerify for Blake3 {
    fn verify(&self, mut reader: impl Read, sig: &[u8]) -> Result<bool> {
        let mut buf: Vec<u8> = Vec::new();
        reader.read_to_end(&mut buf)?;
        let hash = blake3::keyed_hash(&self.key, &buf);
        let hash = hash.as_bytes();
        Ok(hash == sig)
    }
}

impl TextSign for Ed25519Signer {
    fn sign(&self, reader: &mut dyn Read) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let sig = self.key.sign(&buf);
        Ok(sig.to_bytes().to_vec())
    }
}

impl TextVerify for Ed25519Verifier {
    fn verify(&self, mut reader: impl Read, sig: &[u8]) -> Result<bool> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let sig = Signature::from_bytes(sig.try_into()?);
        let res = self.key.verify(&buf, &sig).is_ok();
        Ok(res)
    }
}

impl KeyLoader for Blake3 {
    fn load(path: impl AsRef<Path>) -> Result<Self> {
        let key: Vec<u8> = fs::read(path)?;
        Self::try_new(&key)
    }
}

impl KeyLoader for Ed25519Signer {
    fn load(path: impl AsRef<Path>) -> Result<Self> {
        let key: Vec<u8> = fs::read(path)?;
        Self::try_new(&key)
    }
}

impl KeyLoader for Ed25519Verifier {
    fn load(path: impl AsRef<Path>) -> Result<Self> {
        let key: Vec<u8> = fs::read(path)?;
        Self::try_new(&key)
    }
}

impl KeyGenerator for Blake3 {
    fn generate() -> Result<Vec<Vec<u8>>> {
        let key = process_genpass(32, true, true, true, true)?;
        let key = key.as_bytes().to_vec();  //不是最优写法，但是一次性分配内存
        Ok(vec![key])
    }
}

impl KeyGenerator for Ed25519Signer {
    fn generate() -> Result<Vec<Vec<u8>>> {
        // rand 0.10 中 OsRng 已改名为 SysRng，且它是“可失败”的 TryCryptoRng，
        // 需要用 UnwrapErr 包装后才满足 ed25519-dalek 要求的 CryptoRng
        let mut csprng = UnwrapErr(SysRng);
        let sk = SigningKey::generate(&mut csprng);
        let pk = sk.verifying_key().to_bytes().to_vec();
        let sk = sk.to_bytes().to_vec();
        Ok(vec![sk, pk])
    }
}

impl Blake3 {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
    pub fn try_new(key: &[u8]) -> Result<Self> {
        let key = &key[..32];
        let key = key.try_into()?;
        let signer = Blake3::new(key);
        Ok(signer)
    }
}

impl Ed25519Signer {
    pub fn new(key: SigningKey) -> Self {
        Self { key }
    }
    pub fn try_new(key: &[u8]) -> Result<Self> {
        let key = SigningKey::from_bytes(key.try_into()?);
        let signer = Ed25519Signer::new(key);
        Ok(signer)
    }
}

impl Ed25519Verifier {
    pub fn new(key: VerifyingKey) -> Self {
        Self { key }
    }
    pub fn try_new(key: &[u8]) -> Result<Self> {
        let key = VerifyingKey::from_bytes(key.try_into()?)?;
        let verifier = Ed25519Verifier::new(key);
        Ok(verifier)
    }
}

pub fn process_text_sign(input: &str, key: &str, format: TextSignFormat) -> Result<String> {
    let mut reader = get_reader(input)?;

    let signed = match format {
        TextSignFormat::Blake3 => {
            let signer = Blake3::load(key)?;
            signer.sign(&mut reader)?
        },
        TextSignFormat::Ed25519 => {
            let signer = Ed25519Signer::load(key)?;
            signer.sign(&mut reader)?
        }
    };
    
    let signed = URL_SAFE_NO_PAD.encode(&signed);
    Ok(signed)
}

pub fn process_text_verify(input: &str, key: &str,sig: &str, format: TextSignFormat) -> Result<bool> {
    let mut reader = get_reader(input)?;
    let sig = URL_SAFE_NO_PAD.decode(sig)?;
    let verified = match format {
        TextSignFormat::Blake3 => {
            let verifier = Blake3::load(key)?;
            verifier.verify(&mut reader, &sig)?
        },
        TextSignFormat::Ed25519 => {
            let verifier = Ed25519Verifier::load(key)?;
            verifier.verify(&mut reader, &sig)?
        }
    };
    Ok(verified)
}

pub fn process_text_generate(format: TextSignFormat) -> Result<Vec<Vec<u8>>> {
    match format {
        TextSignFormat::Blake3 => Blake3::generate(),
        TextSignFormat::Ed25519 => Ed25519Signer::generate(),
    }
}

#[test]
fn test_blake3_sign_verify() -> Result<()> {
    let blake3 = Blake3::load("fixtures/blake3.txt")?;
    let data = b"hello world";
    let sig = blake3.sign(&mut & data[..]).unwrap();
    assert!(blake3.verify(&mut & data[..], &sig).unwrap());
    Ok(())
}

#[test]
fn test_ed25519_sign_verfy() -> Result<()> {
    let sk = Ed25519Signer::load("fixtures/ed25519.sk")?;
    let pk = Ed25519Verifier::load("fixtures/ed25519.pk")?;

    let data = b"hello world";
    let sig = sk.sign(&mut &data[..])?;
    assert!(pk.verify(&data[..], &sig)?);
    Ok(())
}