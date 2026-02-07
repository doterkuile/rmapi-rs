use const_format::formatcp;

pub const AUTH_API_URL_ROOT: &str = "https://webapp-prod.cloud.remarkable.engineering";
pub const AUTH_API_VERSION: &str = "2";
pub const NEW_CLIENT_URL: &str =
    formatcp!("{AUTH_API_URL_ROOT}/token/json/{AUTH_API_VERSION}/device/new");
pub const NEW_TOKEN_URL: &str =
    formatcp!("{AUTH_API_URL_ROOT}/token/json/{AUTH_API_VERSION}/user/new");

pub const SERVICE_DISCOVERY_API_URL_ROOT: &str =
    "https://service-manager-production-dot-remarkable-production.appspot.com";
pub const STORAGE_API_VERSION: &str = "1";
pub const STORAGE_DISCOVERY_API_URL: &str = formatcp!(
    "{SERVICE_DISCOVERY_API_URL_ROOT}/service/json/{STORAGE_API_VERSION}/document-storage"
);
pub const GROUP_AUTH: &str = "auth0%7C5a68dc51cb30df1234567890";
pub const STORAGE_DISCOVERY_API_VERSION: &str = "2";

pub const STORAGE_API_URL_ROOT: &str = "https://internal.cloud.remarkable.com";
pub const WEBAPP_API_URL_ROOT: &str = "https://web.eu.tectonic.remarkable.com";

pub const DOC_UPLOAD_ENDPOINT: &str = "doc/v2/files";
pub const ROOT_SYNC_ENDPOINT: &str = "sync/v3/root";

// Headers
pub const HEADER_RM_FILENAME: &str = "rm-filename";
pub const HEADER_RM_SOURCE: &str = "rm-Source";
pub const HEADER_RM_META: &str = "rm-Meta";
pub const HEADER_X_GOOG_HASH: &str = "x-goog-hash";

// MIME Types
pub const MIME_TYPE_PDF: &str = "application/pdf";
pub const MIME_TYPE_JSON: &str = "application/json";
pub const MIME_TYPE_OCTET_STREAM: &str = "application/octet-stream";
pub const MIME_TYPE_DOC_SCHEMA: &str = "text/plain; charset=UTF-8";

// Magic Values
pub const ROOT_ID: &str = "80000000";
pub const TRASH_ID: &str = "de000000-0000-0000-0000-000000000000";
pub const DOC_TYPE_DOCUMENT: &str = "DocumentType";
pub const DOC_TYPE_COLLECTION: &str = "CollectionType";
pub const MSG_UNKNOWN_COUNT_4: &str = "4";
pub const MSG_UNKNOWN_COUNT_0: &str = "0";
