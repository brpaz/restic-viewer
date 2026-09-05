//! Stores and retrieves secrets (Repository Password, Backend credentials) via the
//! system keyring (libsecret), keyed by Repository id. Never touches the config file.

use std::collections::HashMap;

const SCHEMA_NAME: &str = "dev.brunopaz.ResticGtk.Credential";

/// Which secret this is, for a given Repository. Both share one schema, distinguished
/// by this attribute, so a Repository can hold a password and a separate Backend
/// credential (SFTP password, S3 secret key) side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Password,
    BackendSecret,
}

impl CredentialKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::BackendSecret => "backend-secret",
        }
    }
}

fn schema() -> libsecret::Schema {
    libsecret::Schema::new(
        SCHEMA_NAME,
        libsecret::SchemaFlags::NONE,
        HashMap::from([
            ("repository-id", libsecret::SchemaAttributeType::String),
            ("kind", libsecret::SchemaAttributeType::String),
        ]),
    )
}

fn attributes(repository_id: &str, kind: CredentialKind) -> HashMap<&str, &str> {
    HashMap::from([("repository-id", repository_id), ("kind", kind.as_str())])
}

pub async fn store(
    repository_id: &str,
    kind: CredentialKind,
    secret: &str,
    label: &str,
) -> Result<(), glib::Error> {
    libsecret::password_store_future(
        Some(&schema()),
        attributes(repository_id, kind),
        None,
        label,
        secret,
    )
    .await
}

pub async fn lookup(
    repository_id: &str,
    kind: CredentialKind,
) -> Result<Option<String>, glib::Error> {
    let result =
        libsecret::password_lookup_future(Some(&schema()), attributes(repository_id, kind)).await?;
    Ok(result.map(|s| s.to_string()))
}

pub async fn delete(repository_id: &str, kind: CredentialKind) -> Result<(), glib::Error> {
    libsecret::password_clear_future(Some(&schema()), attributes(repository_id, kind)).await
}
