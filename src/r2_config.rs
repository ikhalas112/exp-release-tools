use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;

#[derive(Debug, Clone)]
pub struct R2Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket_name: String,
    pub endpoint: String,
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn embedded_r2(field: &str) -> Option<String> {
    match field {
        "R2_ACCESS_KEY_ID" => non_empty(env!("EMBED_R2_ACCESS_KEY_ID")),
        "R2_SECRET_ACCESS_KEY" => non_empty(env!("EMBED_R2_SECRET_ACCESS_KEY")),
        "R2_BUCKET_NAME" => non_empty(env!("EMBED_R2_BUCKET_NAME")),
        "R2_ENDPOINT" => non_empty(env!("EMBED_R2_ENDPOINT")),
        _ => None,
    }
}

fn resolve_field(name: &str) -> Result<String> {
    if let Ok(v) = std::env::var(name) {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    embedded_r2(name).ok_or_else(|| anyhow::anyhow!("missing {name}"))
}

pub fn r2_credentials() -> Result<R2Credentials> {
    Ok(R2Credentials {
        access_key_id: resolve_field("R2_ACCESS_KEY_ID")?,
        secret_access_key: resolve_field("R2_SECRET_ACCESS_KEY")?,
        bucket_name: resolve_field("R2_BUCKET_NAME")?,
        endpoint: resolve_field("R2_ENDPOINT")?,
    })
}

pub fn maxion_build_secret() -> Option<String> {
    if let Ok(v) = std::env::var("MAXION_BUILD_SECRET") {
        if !v.is_empty() {
            return Some(v);
        }
    }
    non_empty(env!("EMBED_MAXION_BUILD_SECRET"))
}

pub async fn s3_client() -> Result<Client> {
    let creds = r2_credentials()?;
    let aws_creds = Credentials::new(
        creds.access_key_id,
        creds.secret_access_key,
        None,
        None,
        "embedded-or-env",
    );
    let cfg = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(aws_creds)
        .endpoint_url(creds.endpoint)
        .region("auto")
        .load()
        .await;
    Ok(Client::from_conf(
        aws_sdk_s3::config::Builder::from(&cfg)
            .force_path_style(true)
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_env_overrides_embedded() {
        std::env::set_var("R2_ACCESS_KEY_ID", "test-key");
        std::env::set_var("R2_SECRET_ACCESS_KEY", "test-secret");
        std::env::set_var("R2_BUCKET_NAME", "test-bucket");
        std::env::set_var("R2_ENDPOINT", "https://example.r2.cloudflarestorage.com");
        let creds = r2_credentials().unwrap();
        assert_eq!(creds.access_key_id, "test-key");
        assert_eq!(creds.bucket_name, "test-bucket");
        std::env::remove_var("R2_ACCESS_KEY_ID");
        std::env::remove_var("R2_SECRET_ACCESS_KEY");
        std::env::remove_var("R2_BUCKET_NAME");
        std::env::remove_var("R2_ENDPOINT");
    }

    #[test]
    fn maxion_secret_from_env() {
        std::env::set_var("MAXION_BUILD_SECRET", "dev-secret");
        assert_eq!(maxion_build_secret().as_deref(), Some("dev-secret"));
        std::env::remove_var("MAXION_BUILD_SECRET");
    }
}
