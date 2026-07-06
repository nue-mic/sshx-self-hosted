use anyhow::Result;
use sshx::encrypt::Encrypt;
use sshx_core::proto::*;

use crate::common::*;

pub mod common;

#[tokio::test]
async fn test_rpc() -> Result<()> {
    let server = TestServer::new().await;
    let mut client = server.grpc_client().await;

    let req = OpenRequest {
        origin: "sshx.io".into(),
        encrypted_zeros: Encrypt::new("").zeros().into(),
        name: String::new(),
        write_password_hash: None,
        session_id: String::new(),
    };
    let resp = client.open(req).await?;
    assert!(!resp.into_inner().name.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_fixed_session_id() -> Result<()> {
    let server = TestServer::new().await;
    let mut client = server.grpc_client().await;

    let zeros: Vec<u8> = Encrypt::new("secret-key").zeros();

    // A client-provided session ID is used verbatim as the session name/URL.
    let resp = client
        .open(OpenRequest {
            origin: "sshx.io".into(),
            encrypted_zeros: zeros.clone().into(),
            name: String::new(),
            write_password_hash: None,
            session_id: "myfixedid1".into(),
        })
        .await?
        .into_inner();
    assert_eq!(resp.name, "myfixedid1");
    assert!(resp.url.ends_with("/s/myfixedid1"), "url was {}", resp.url);

    // Reopening with the same ID and the same encryption key reclaims it.
    let resp2 = client
        .open(OpenRequest {
            origin: "sshx.io".into(),
            encrypted_zeros: zeros.clone().into(),
            name: String::new(),
            write_password_hash: None,
            session_id: "myfixedid1".into(),
        })
        .await?
        .into_inner();
    assert_eq!(resp2.name, "myfixedid1");

    // Reopening with the same ID but a different encryption key is rejected.
    let other = Encrypt::new("different-key").zeros();
    let err = client
        .open(OpenRequest {
            origin: "sshx.io".into(),
            encrypted_zeros: other.into(),
            name: String::new(),
            write_password_hash: None,
            session_id: "myfixedid1".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::PermissionDenied);

    // A malformed session ID is rejected.
    let err = client
        .open(OpenRequest {
            origin: "sshx.io".into(),
            encrypted_zeros: zeros.into(),
            name: String::new(),
            write_password_hash: None,
            session_id: "bad id!".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    Ok(())
}

#[tokio::test]
async fn test_web_get() -> Result<()> {
    let server = TestServer::new().await;

    let resp = reqwest::get(server.endpoint()).await?;
    assert!(!resp.status().is_server_error());

    Ok(())
}
