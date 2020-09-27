use biscuit::{jwa, jws, ClaimsSet, RegisteredClaims, SingleOrMultiple, JWT};
use chrono::Utc;
use snafu::ResultExt;
use std::str::FromStr;

use crate::auth;
use crate::error;
use crate::settings::Settings;

// type DateTimeUtc = chrono::DateTime<chrono::Utc>;

#[derive(Clone, Debug)]
pub struct Jwt {
    secret: String,
    duration: chrono::Duration,
}

impl Jwt {
    pub fn new(settings: &Settings) -> Self {
        Self {
            secret: String::from(&settings.jwt.secret),
            duration: chrono::Duration::minutes(settings.jwt.duration),
        }
    }

    pub fn encode(&self, claims: auth::PrivateClaims) -> Result<String, error::Error> {
        let expiry = Utc::now() + self.duration;
        let registered = RegisteredClaims {
            issuer: Some(FromStr::from_str("https://www.acme.com").unwrap()),
            subject: Some(FromStr::from_str("John Doe").unwrap()),
            audience: Some(SingleOrMultiple::Single(
                FromStr::from_str("htts://acme-customer.com").unwrap(),
            )),
            expiry: Some(expiry.into()),
            ..Default::default()
        };
        let private = claims;
        let claims = ClaimsSet::<auth::PrivateClaims> {
            registered,
            private,
        };

        let jwt = biscuit::JWT::new_decoded(
            From::from(jws::RegisteredHeader {
                algorithm: jwa::SignatureAlgorithm::HS256,
                ..Default::default()
            }),
            claims,
        );

        let secret = jws::Secret::bytes_from_str(&self.secret);

        jwt.into_encoded(&secret)
            .map(|t| t.unwrap_encoded().to_string())
            .context(error::BiscuitError {
                msg: String::from("could not encode jwt"),
            })
    }

    pub fn decode(
        &self,
        token: &str,
    ) -> Result<biscuit::ClaimsSet<auth::PrivateClaims>, error::Error> {
        let token = JWT::<auth::PrivateClaims, biscuit::Empty>::new_encoded(&token);
        let secret = jws::Secret::bytes_from_str(&self.secret);
        let token = token
            .into_decoded(&secret, jwa::SignatureAlgorithm::HS256)
            .context(error::BiscuitError {
                msg: String::from("could not decode jwt"),
            })?;
        let payload = token
            .payload()
            .context(error::BiscuitError {
                msg: String::from("could not get jwt payload"),
            })?
            //.private
            .to_owned();
        Ok(payload)
    }
}
