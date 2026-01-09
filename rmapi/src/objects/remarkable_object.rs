use crate::error::Error;

pub trait RemarkableObject {
    fn register_client(code: String) -> Result<String, Error>;
}
