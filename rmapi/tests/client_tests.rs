use rmapi::objects::{Document, DocumentType, RootInfo};
use rmapi::RmClient;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Creates a fresh `RmClient` pointed at the given mock server, with an empty filesystem cache.
async fn create_test_client(base_url: &str) -> RmClient {
    let mut client = RmClient::new(
        "device_token",
        Some("user_token"),
        Some(base_url),
        Some(base_url),
        Some(base_url),
    )
    .await
    .expect("Failed to create test client");
    client.filesystem = rmapi::filesystem::FileSystem::new();
    client
}

/// Mounts a mock for `GET /sync/v3/root` returning the given root hash and generation.
async fn mock_root_info(server: &MockServer, root_hash: &str, generation: u64) {
    let root_info = RootInfo {
        hash: root_hash.to_string(),
        generation,
    };
    Mock::given(method("GET"))
        .and(path("/sync/v3/root"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&root_info))
        .mount(server)
        .await;
}

/// Mounts a mock for `GET /sync/v3/files/{hash}` returning the given body string.
async fn mock_blob(server: &MockServer, hash: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/sync/v3/files/{}", hash)))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Mounts a mock for `GET /sync/v3/files/{hash}` returning raw bytes.
async fn mock_blob_bytes(server: &MockServer, hash: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(format!("/sync/v3/files/{}", hash)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

/// Mounts a mock for `GET /sync/v3/files/{hash}` returning JSON.
async fn mock_blob_json(server: &MockServer, hash: &str, json: &serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(format!("/sync/v3/files/{}", hash)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json))
        .mount(server)
        .await;
}

/// Mounts a mock for `PUT /sync/v3/root` accepting root updates.
async fn mock_root_update(server: &MockServer) {
    Mock::given(method("PUT"))
        .and(path("/sync/v3/root"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_files() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let mut client = create_test_client(&base_url).await;

    let root_hash = "root_hash_123";
    let doc_id = "00000000-0000-0000-0000-000000000001";
    let doc_hash = "doc_hash_1";
    let metadata_hash = "metadata_hash_1";

    mock_root_info(&mock_server, root_hash, 1).await;
    mock_blob(
        &mock_server,
        root_hash,
        &format!("3\n{}:DocumentType:{}:1:100", doc_hash, doc_id),
    )
    .await;
    mock_blob(
        &mock_server,
        doc_hash,
        &format!("3\n{}:metadata:{}.metadata:0:50", metadata_hash, doc_id),
    )
    .await;
    mock_blob_json(
        &mock_server,
        metadata_hash,
        &serde_json::json!({
            "visibleName": "Test Document",
            "type": "DocumentType",
            "parent": "",
            "lastModified": "1600000000000",
            "version": 1,
            "pinned": false,
            "deleted": false
        }),
    )
    .await;

    let docs = client.list_files().await.expect("Failed to list files");

    assert_eq!(docs.len(), 1);
    let doc = &docs[0];
    assert_eq!(doc.display_name, "Test Document");
    assert_eq!(doc.id.to_string(), doc_id);
    assert_eq!(doc.doc_type, DocumentType::Document);
    assert!(!doc.bookmarked);
}

#[tokio::test]
async fn test_refresh_token() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();

    let device_token = "valid_device_token";
    let new_user_token = "new_user_token";

    Mock::given(method("POST"))
        .and(path("/token/json/2/user/new"))
        .and(header(
            "Authorization",
            format!("Bearer {}", device_token).as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(new_user_token))
        .mount(&mock_server)
        .await;

    let client = RmClient::new(
        device_token,
        None,
        Some(&base_url),
        Some(&base_url),
        Some(&base_url),
    )
    .await
    .expect("Failed to create client");

    assert_eq!(client.user_token, new_user_token);
}

#[tokio::test]
async fn test_check_authentication_success() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let mut client = create_test_client(&base_url).await;

    let root_hash = "auth_root_hash";

    // check_authentication calls list_files, which calls get_root_info + get_files.
    // For success, we just need the root info to return something,
    // and get_files to succeed minimally (empty root index).
    mock_root_info(&mock_server, root_hash, 1).await;
    mock_blob(&mock_server, root_hash, "3\n").await;

    let result = client.check_authentication().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_authentication_failure() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let mut client = create_test_client(&base_url).await;

    // Mock root info returning 401 Unauthorized
    Mock::given(method("GET"))
        .and(path("/sync/v3/root"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let result = client.check_authentication().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_download_document_pdf() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let client = create_test_client(&base_url).await;

    let doc_id_str = "00000000-0000-0000-0000-000000000002";
    let doc_id = Uuid::parse_str(doc_id_str).unwrap();

    let root_hash = "dl_root_hash";
    let doc_entry_hash = "dl_doc_entry_hash";
    let pdf_blob_hash = "aabbccdd00112233aabbccdd00112233aabbccdd00112233aabbccdd00112233";
    let metadata_blob_hash = "eeff00112233445566778899aabbccddeeff00112233445566778899aabbccdd";

    // 1. Mock root info
    mock_root_info(&mock_server, root_hash, 1).await;

    // 2. Mock root index blob (contains our doc entry)
    let root_index = format!("3\n{}:80000000:{}:0:500", doc_entry_hash, doc_id_str);
    mock_blob(&mock_server, root_hash, &root_index).await;

    // 3. Mock doc schema (contains a .pdf and .metadata subfile)
    let doc_schema = format!(
        "3\n{}:0:{}.pdf:0:100\n{}:0:{}.metadata:0:50",
        pdf_blob_hash, doc_id_str, metadata_blob_hash, doc_id_str
    );
    mock_blob(&mock_server, doc_entry_hash, &doc_schema).await;

    // 4. Mock PDF blob
    let pdf_content = b"%PDF-1.4 fake pdf content for testing";
    mock_blob_bytes(&mock_server, pdf_blob_hash, pdf_content.to_vec()).await;

    // 5. Download
    let tmp_dir = std::env::temp_dir().join("rmapi_test_download");
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .expect("Failed to create temp dir");
    let target = tmp_dir.join("test_doc");

    let result = client
        .download_document(&doc_id, &target)
        .await
        .expect("Failed to download document");

    // 6. Verify
    assert_eq!(result.extension().unwrap(), "pdf");
    let downloaded_content = tokio::fs::read(&result).await.expect("Failed to read file");
    assert_eq!(downloaded_content, pdf_content);

    // Cleanup
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
}

#[tokio::test]
async fn test_delete_entry() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let client = create_test_client(&base_url).await;

    let doc_id_str = "00000000-0000-0000-0000-000000000003";
    let doc_id = Uuid::parse_str(doc_id_str).unwrap();
    let root_hash = "del_root_hash";
    let doc_entry_hash = "1111111111111111111111111111111111111111111111111111111111111111";

    // 1. Mock root info
    mock_root_info(&mock_server, root_hash, 5).await;

    // 2. Mock root index blob with one entry
    let root_index = format!("3\n{}:80000000:{}:0:100", doc_entry_hash, doc_id_str);
    mock_blob(&mock_server, root_hash, &root_index).await;

    // 3. Mock upload for new root blob (any PUT to /sync/v3/files/*)
    Mock::given(method("PUT"))
        .and(wiremock::matchers::path_regex(r"^/sync/v3/files/.+$"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // 4. Mock root update
    mock_root_update(&mock_server).await;

    // 5. Create a Document object for delete_entry
    let doc = Document {
        id: doc_id,
        version: 1,
        message: String::new(),
        success: true,
        blob_url_get: String::new(),
        blob_url_put: String::new(),
        blob_url_put_expires: chrono::Utc::now(),
        last_modified: chrono::Utc::now(),
        doc_type: DocumentType::Document,
        display_name: "To Be Deleted".to_string(),
        current_page: 0,
        bookmarked: false,
        parent: String::new(),
    };

    let result = client.delete_entry(&doc).await;
    assert!(result.is_ok(), "delete_entry failed: {:?}", result.err());
}

#[tokio::test]
async fn test_list_files_with_multiple_documents() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let mut client = create_test_client(&base_url).await;

    let root_hash = "multi_root_hash";
    let doc_id_1 = "00000000-0000-0000-0000-000000000010";
    let doc_id_2 = "00000000-0000-0000-0000-000000000020";
    let doc_hash_1 = "multi_doc_hash_1";
    let doc_hash_2 = "multi_doc_hash_2";
    let meta_hash_1 = "multi_meta_hash_1";
    let meta_hash_2 = "multi_meta_hash_2";

    mock_root_info(&mock_server, root_hash, 1).await;

    // Root index with two docs
    let root_index = format!(
        "3\n{}:DocumentType:{}:1:100\n{}:CollectionType:{}:1:50",
        doc_hash_1, doc_id_1, doc_hash_2, doc_id_2
    );
    mock_blob(&mock_server, root_hash, &root_index).await;

    // Doc schemas
    mock_blob(
        &mock_server,
        doc_hash_1,
        &format!("3\n{}:0:{}.metadata:0:50", meta_hash_1, doc_id_1),
    )
    .await;
    mock_blob(
        &mock_server,
        doc_hash_2,
        &format!("3\n{}:0:{}.metadata:0:50", meta_hash_2, doc_id_2),
    )
    .await;

    // Metadata for doc1 (a document)
    mock_blob_json(
        &mock_server,
        meta_hash_1,
        &serde_json::json!({
            "visibleName": "My PDF",
            "type": "DocumentType",
            "parent": "",
            "lastModified": "1700000000000",
            "version": 2,
            "pinned": true,
            "deleted": false
        }),
    )
    .await;

    // Metadata for doc2 (a collection/folder)
    mock_blob_json(
        &mock_server,
        meta_hash_2,
        &serde_json::json!({
            "visibleName": "My Folder",
            "type": "CollectionType",
            "parent": "",
            "lastModified": "1700000000000",
            "version": 1,
            "pinned": false,
            "deleted": false
        }),
    )
    .await;

    let docs = client.list_files().await.expect("Failed to list files");

    assert_eq!(docs.len(), 2);

    // Find docs by name (order may vary due to concurrency)
    let pdf_doc = docs.iter().find(|d| d.display_name == "My PDF").unwrap();
    let folder_doc = docs.iter().find(|d| d.display_name == "My Folder").unwrap();

    assert_eq!(pdf_doc.doc_type, DocumentType::Document);
    assert!(pdf_doc.bookmarked);
    assert_eq!(pdf_doc.version, 2);

    assert_eq!(folder_doc.doc_type, DocumentType::Collection);
    assert!(!folder_doc.bookmarked);
}

#[tokio::test]
async fn test_list_files_filters_deleted() {
    let mock_server = MockServer::start().await;
    let base_url = mock_server.uri();
    let mut client = create_test_client(&base_url).await;

    let root_hash = "deleted_root_hash";
    let doc_id = "00000000-0000-0000-0000-000000000099";
    let doc_hash = "deleted_doc_hash";
    let meta_hash = "deleted_meta_hash";

    mock_root_info(&mock_server, root_hash, 1).await;
    mock_blob(
        &mock_server,
        root_hash,
        &format!("3\n{}:DocumentType:{}:1:100", doc_hash, doc_id),
    )
    .await;
    mock_blob(
        &mock_server,
        doc_hash,
        &format!("3\n{}:0:{}.metadata:0:50", meta_hash, doc_id),
    )
    .await;

    // Metadata with deleted = true
    mock_blob_json(
        &mock_server,
        meta_hash,
        &serde_json::json!({
            "visibleName": "Deleted Doc",
            "type": "DocumentType",
            "parent": "",
            "lastModified": "1600000000000",
            "version": 1,
            "pinned": false,
            "deleted": true
        }),
    )
    .await;

    let docs = client.list_files().await.expect("Failed to list files");
    assert_eq!(docs.len(), 0, "Deleted documents should be filtered out");
}
