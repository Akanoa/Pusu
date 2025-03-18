use crate::errors::PusuServerError;
use biscuit_auth::macros::{authorizer, rule};
use biscuit_auth::{AuthorizerLimits, Biscuit, PublicKey};
use std::time::Duration;

/// Authorizes a Biscuit token and extracts the tenant information.
///
/// This function takes a base64-encoded Biscuit token and a public key.
/// It validates the token and checks its authorizations against predefined rules.
/// If the token includes a valid `tenant` fact, the function extracts and returns
/// the tenant string. If the tenant fact is missing or the token is invalid,
/// an error is returned.
///
/// # Arguments
///
/// * `biscuit_base_64` - A base64-encoded string representing the Biscuit token.
/// * `public_key` - The public key used to verify the Biscuit token.
///
/// # Returns
///
/// * `Ok(String)` - Returns the tenant as a string if authorization succeeds and the tenant fact is found.
/// * `Err(crate::errors::PlushyError)` - Returns an error if the token is invalid, missing required facts, or fails authorization.
///
/// # Errors
///
/// This function can return the following error variants:
/// * `PlushyError::MalformedBiscuitMissingTenantFact` - If the token is valid but does not contain the expected `tenant` fact.
/// * Other variants propagated from internal validation failures of the Biscuit library.
pub fn authorize(biscuit_base_64: &str, public_key: &PublicKey) -> crate::errors::Result<String> {
    let biscuit = Biscuit::from_base64(biscuit_base_64, public_key)?;

    let mut authorizer = authorizer!(
        r#"
        allow if true;
    "#
    );

    let mut run_limits = AuthorizerLimits::default();
    run_limits.max_time = Duration::from_millis(100);

    authorizer.add_token(&biscuit)?;
    authorizer.authorize_with_limits(AuthorizerLimits::default())?;

    let query = rule!(
        r#"
        _($tenant) <- tenant($tenant);
    "#
    );

    let mut data: Vec<(String,)> = authorizer.query(query)?;

    match data.pop() {
        None => Err(PusuServerError::MalformedBiscuitMissingTenantFact),
        Some((tenant,)) => Ok(tenant),
    }
}

#[cfg(test)]
pub mod tests {

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
    pub fn create_biscuit(tenant_id: &str) -> crate::errors::Result<(Biscuit, KeyPair)> {
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
    #[cfg(test)]
    pub fn create_biscuit_with_keypair(
        tenant_id: &str,
        keypair: &KeyPair,
    ) -> crate::errors::Result<Biscuit> {
        let biscuit = biscuit!(
            r#"
        tenant({tenant_id});
        "#,
            tenant_id = tenant_id
        )
        .build(keypair)?;
        Ok(biscuit)
    }

    use super::*;
    use biscuit_auth::macros::biscuit;
    use biscuit_auth::{KeyPair, PublicKey};

    /// Tests the `authorize` function with a valid Biscuit token that includes a tenant fact.
    /// Verifies that the function correctly extracts the tenant and returns it.
    #[test]
    fn test_authorize() {
        // Generate a key pair for testing
        let keypair = KeyPair::new();
        let public_key: PublicKey = keypair.public();

        // Create a biscuit containing a tenant fact
        let biscuit = biscuit!(
            r#"
        tenant("test_tenant");
    "#
        )
        .build(&keypair)
        .expect("Failed to build biscuit");

        // Serialize the biscuit to a base64 string for testing
        let biscuit_base_64 = biscuit.to_base64().unwrap();

        // Call the authorize function and verify the result
        let result = authorize(&biscuit_base_64, &public_key);

        assert!(result.is_ok(), "Authorization failed");
        assert_eq!(result.unwrap(), "test_tenant", "Incorrect tenant returned");
    }

    /// Tests the `authorize` function with a malformed Biscuit token.
    /// Verifies that the function returns an error when the token lacks the required structure.
    #[test]
    fn test_authorize_with_malformed_biscuit() {
        // Generate a key pair for testing
        let keypair = KeyPair::new();
        let public_key: PublicKey = keypair.public();

        // Create a malformed Biscuit token by missing the required tenant fact
        let malformed_biscuit = biscuit!(
            r#"
        fact("invalid_fact", "wrong_value");
    "#
        )
        .build(&keypair)
        .expect("Failed to build malformed biscuit");

        // Serialize the malformed biscuit to a base64 string
        let malformed_biscuit_base_64 = malformed_biscuit.to_base64().unwrap();

        // Call the authorize function with the malformed biscuit
        let result = authorize(&malformed_biscuit_base_64, &public_key);

        // Assert that the result is an error
        assert!(
            result.is_err(),
            "Expected an error for malformed Biscuit, but got an Ok result"
        );

        // Optionally, check for a specific error if applicable
        if let Err(e) = result {
            println!("Received expected error: {:?}", e);
        }
    }

    /// Tests the `authorize` function with a Biscuit token that lacks the tenant fact.
    /// Verifies that the function returns an error when the tenant fact is missing.
    #[test]
    fn test_authorize_without_tenant_fact() {
        // Generate a key pair for testing
        let keypair = KeyPair::new();
        let public_key: PublicKey = keypair.public();

        // Create a biscuit without the tenant fact
        let biscuit_without_tenant = biscuit!(
            r#"
        fact("some_other_fact", "value");
    "#
        )
        .build(&keypair)
        .expect("Failed to build biscuit without tenant fact");

        // Serialize the biscuit to a base64 string
        let biscuit_base_64 = biscuit_without_tenant.to_base64().unwrap();

        // Call the authorize function with the biscuit that lacks the tenant fact
        let result = authorize(&biscuit_base_64, &public_key);

        // Assert that the result is an error
        assert!(
            result.is_err(),
            "Expected an error for biscuit without tenant fact, but got an Ok result"
        );

        // Optionally, check for the specific error, e.g., missing tenant fact
        if let Err(e) = result {
            println!("Received expected error: {:?}", e);
        }
    }
}
