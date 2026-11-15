use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{Args, Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use trust_infra_cli::{CheckStatus, Report, Verdict, sign_bundle, verify};

#[derive(Debug, Parser)]
#[command(
    name = "trustinfra",
    about = "Sign and verify trust infrastructure bundles"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate an Ed25519 keypair.
    Keygen {
        /// Output prefix for the .key and .pub files.
        out_prefix: PathBuf,
    },
    /// Sign a bundle manifest and its artifacts.
    Sign(SignArgs),
    /// Verify a bundle against a trusted public key.
    Verify(VerifyArgs),
}

#[derive(Debug, Args)]
struct SignArgs {
    /// Bundle directory containing manifest.json.
    bundle_dir: PathBuf,
    /// File containing the base64-encoded 32-byte secret key.
    #[arg(long, short = 'k')]
    key: PathBuf,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    /// Bundle directory containing manifest.json.
    bundle_dir: PathBuf,
    /// File containing the base64-encoded 32-byte trusted public key.
    #[arg(long)]
    trusted_key: PathBuf,
    /// Print the report as pretty JSON.
    #[arg(long, help = "Print the verification report as pretty JSON")]
    json: bool,
}

pub fn run() -> i32 {
    match Cli::parse().command {
        Command::Keygen { out_prefix } => match keygen(&out_prefix) {
            Ok((key_path, public_path, public_key)) => {
                println!("{}", key_path.display());
                println!("{}", public_path.display());
                println!("{public_key}");
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        },
        Command::Sign(args) => match sign(&args.bundle_dir, &args.key) {
            Ok(()) => {
                println!("signed {}", args.bundle_dir.display());
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        },
        Command::Verify(args) => {
            let key = match read_verifying_key(&args.trusted_key) {
                Ok(key) => key,
                Err(error) => {
                    eprintln!("{error}");
                    return 2;
                }
            };
            let report = verify(&args.bundle_dir, &key);
            if args.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("{error}");
                        return 2;
                    }
                }
            } else {
                print_human_report(&report);
            }
            match report.verdict {
                Verdict::Verified => 0,
                Verdict::Tampered => 1,
                Verdict::Unverifiable => 2,
            }
        }
    }
}

fn keygen(prefix: &Path) -> Result<(PathBuf, PathBuf, String), Box<dyn std::error::Error>> {
    let key_path = suffixed_path(prefix, ".key");
    let public_path = suffixed_path(prefix, ".pub");
    let key = SigningKey::generate(&mut rand::rng());
    let secret_contents = format!("{}\n", encode_key(&key.to_bytes()));
    let public_key = encode_key(&key.verifying_key().to_bytes());
    let public_contents = format!("{public_key}\n");

    let mut secret_file = create_secret_file(&key_path)?;
    if let Err(error) = secret_file.write_all(secret_contents.as_bytes()) {
        drop(secret_file);
        let _ = fs::remove_file(&key_path);
        return Err(error.into());
    }
    drop(secret_file);

    let mut public_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&public_path)
    {
        Ok(file) => file,
        Err(error) => {
            let _ = fs::remove_file(&key_path);
            return Err(error.into());
        }
    };
    if let Err(error) = public_file.write_all(public_contents.as_bytes()) {
        drop(public_file);
        let _ = fs::remove_file(&key_path);
        let _ = fs::remove_file(&public_path);
        return Err(error.into());
    }

    Ok((key_path, public_path, public_key))
}

fn create_secret_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn sign(bundle_dir: &Path, key_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let key = read_signing_key(key_file)?;
    sign_bundle(bundle_dir, &key)?;
    Ok(())
}

fn read_signing_key(path: &Path) -> Result<SigningKey, Box<dyn std::error::Error>> {
    let bytes = decode_key_file(path)?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "secret key must decode to exactly 32 bytes")?;
    Ok(SigningKey::from_bytes(&seed))
}

fn read_verifying_key(path: &Path) -> Result<VerifyingKey, Box<dyn std::error::Error>> {
    let bytes = decode_key_file(path)?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "public key must decode to exactly 32 bytes")?;
    Ok(VerifyingKey::from_bytes(&key_bytes)?)
}

fn decode_key_file(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    Ok(decode_key(contents.trim())?)
}

fn encode_key(bytes: &[u8; 32]) -> String {
    STANDARD.encode(bytes)
}

fn decode_key(contents: &str) -> Result<Vec<u8>, base64::DecodeError> {
    STANDARD.decode(contents.trim())
}

fn suffixed_path(prefix: &Path, suffix: &str) -> PathBuf {
    let mut path = prefix.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

fn print_human_report(report: &Report) {
    println!("VERDICT: {}", verdict_text(report.verdict));
    for check in &report.checks {
        let status = match check.status {
            CheckStatus::Pass => "PASS",
            CheckStatus::Fail => "FAIL",
        };
        println!("[{status}] {}: {}", check.id, check.detail);
    }
    if !report.infra_diff.is_empty() {
        println!();
        println!("infra diff:");
        for entry in &report.infra_diff {
            println!(
                "  {}: declared={} actual={}",
                entry.path,
                entry.declared.as_deref().unwrap_or("(absent)"),
                entry.actual.as_deref().unwrap_or("(absent)")
            );
        }
    }
}

fn verdict_text(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Verified => "VERIFIED",
        Verdict::Tampered => "TAMPERED",
        Verdict::Unverifiable => "UNVERIFIABLE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_encode_decode_round_trip() {
        let key = SigningKey::generate(&mut rand::rng());
        let encoded = encode_key(&key.to_bytes());
        let decoded: [u8; 32] = decode_key(&encoded).unwrap().try_into().unwrap();
        assert_eq!(decoded, key.to_bytes());
    }

    #[test]
    fn rejects_wrong_length_keys() {
        let encoded = STANDARD.encode([1_u8; 31]);
        assert!(
            decode_key(&encoded)
                .unwrap()
                .try_into()
                .map(|_: [u8; 32]| ())
                .is_err()
        );
    }

    #[test]
    fn trims_whitespace_around_key_data() {
        let key = [7_u8; 32];
        let encoded = encode_key(&key);
        let decoded: [u8; 32] = decode_key(&format!(" \n{encoded}\t\r\n"))
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(decoded, key);
    }
}
