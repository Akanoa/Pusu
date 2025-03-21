use crate::cli::{BiscuitArgs, Command};
use biscuit_auth::macros::biscuit;
use biscuit_auth::{Biscuit, KeyPair, PrivateKey};
use clap::Parser;

mod cli;
pub mod errors;
pub mod test_server;

pub fn run() -> errors::Result<()> {
    let cli = cli::Cli::parse();
    match cli.commnd {
        Command::Biscuit(args) => generate_biscuit(args),
    }
}

/// Generates a Biscuit token for a given tenant, optionally using an existing private key.
///
/// This function creates a Biscuit token and a keypair based on the provided `BiscuitArgs`.
/// If a private key is supplied in the arguments, it is used to generate the token.
/// If no private key is supplied, a new keypair is created. The Biscuit token and the key details
/// are printed to the console for debugging or informational purposes.
///
/// # Arguments
///
/// * `args` - Contains the tenant identifier and an optional private key in hexadecimal format.
///
/// # Returns
///
/// * `Ok(())` - Indicates that the Biscuit token and keypair were successfully generated and displayed.
/// * `Err(crate::errors::PlushyError)` - Returns an error if the Biscuit token generation fails,
///   such as invalid key input or issues during signing.
///
/// # Errors
///
/// This function can return the following error variants:
/// * Errors related to parsing the provided private key.
/// * Errors that occur during Biscuit token creation, such as invalid tenant facts or signing issues.
fn generate_biscuit(args: BiscuitArgs) -> errors::Result<()> {
    let (biscuit, keypair) = if let Some(private_key_hex) = args.key {
        let private_key = PrivateKey::from_bytes_hex(&private_key_hex)?;
        let keypair = KeyPair::from(&private_key);
        let biscuit = create_biscuit_with_keypair(&args.tenant, &keypair)?;
        (biscuit, keypair)
    } else {
        let (biscuit, keypair) = create_biscuit(&args.tenant)?;
        (biscuit, keypair)
    };

    let revocation_ids = biscuit
        .revocation_identifiers()
        .iter()
        .map(hex::encode)
        .collect::<Vec<String>>();

    println!("Biscuit token: {}", biscuit.to_base64()?);
    println!("Private key: {}", keypair.private().to_bytes_hex());
    println!("Public key: {}", keypair.public().to_bytes_hex());
    println!("Tenant: {}", args.tenant);
    print!("Revocation ID : {:?}", revocation_ids);

    Ok(())
}

/// Creates a new Biscuit token with a specific tenant fact.
///
/// This function generates a Biscuit token with a tenant fact,
/// using the provided tenant identifier. It also creates and returns
/// a new key pair used to sign the token.
///
/// # Arguments
///
/// * `tenant_id` - A string slice representing the tenant identifier to include in the token.
///
/// # Returns
///
/// * `Ok((Biscuit, KeyPair))` - A tuple containing the generated Biscuit token and the key pair.
/// * `Err(crate::errors::PlushyError)` - Returns an error if the token creation fails.
///
/// # Errors
///
/// This function can return the following error variants:
/// * Errors related to Biscuit creation, such as invalid data or signing issues.
pub fn create_biscuit(tenant_id: &str) -> errors::Result<(Biscuit, KeyPair)> {
    let keypair = KeyPair::new();
    let biscuit = biscuit!(
        r#"
        tenant({tenant_id});
        "#,
        tenant_id = tenant_id
    )
    .build(&keypair)?;
    Ok((biscuit, keypair))
}

/// Creates a Biscuit token with a specific tenant fact using an existing key pair.
///
/// This function generates a Biscuit token by including a tenant identifier as a fact.
/// The token is signed using the provided key pair.
///
/// # Arguments
///
/// * `tenant_id` - A string slice representing the tenant identifier to be included in the token.
/// * `keypair` - A reference to the existing `KeyPair` used for signing the Biscuit token.
///
/// # Returns
///
/// * `Ok(Biscuit)` - The constructed Biscuit token containing the tenant fact.
/// * `Err(crate::errors::PlushyError)` - Returns an error if the Biscuit creation fails, such as
///   due to invalid input or signing issues.
///
/// # Errors
///
/// This function can return the following error variants:
/// * Errors related to Biscuit creation, such as invalid data or issues with signing the token.
pub fn create_biscuit_with_keypair(tenant_id: &str, keypair: &KeyPair) -> errors::Result<Biscuit> {
    let biscuit = biscuit!(
        r#"
        tenant({tenant_id});
        "#,
        tenant_id = tenant_id
    )
    .build(keypair)?;
    Ok(biscuit)
}
