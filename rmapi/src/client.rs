use crate::endpoints;

use crate::error::Error;

use tokio::fs::File;

use log;

/// Represents a client for interacting with the reMarkable Cloud API.
///
/// This struct holds the authentication token, the path to the token file,
/// and an optional storage URL for the client.
pub struct Client {
    /// The authentication token used for API requests.
    pub auth_token: String,
    /// An optional URL for the storage API endpoint.
    pub storage_url: String,
}

/// TODO: Token caching in library or in the app (feels like app but so many operations need to be atomic)?
impl Client {
    /// Creates a new `Client` instance from an existing auth token.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - A string slice containing the authentication token.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// - `Ok(Client)`: A new `Client` instance with the provided token.
    /// - `Err(Error)`: An error if writing the token file fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if writing the token file fails.
    pub async fn from_token(auth_token: &str) -> Result<Client, Error> {
        log::debug!("New client with auth token: {:?}", auth_token);
        //let storage_url = endpoints::discover_storage(auth_token).await?;
        Ok(Client {
            auth_token: auth_token.to_string(),
            storage_url: endpoints::STORAGE_API_URL_ROOT.to_string(),
        })
    }

    /// Creates a new `Client` instance by registering with the reMarkable Cloud using a provided code.
    ///
    /// # Arguments
    ///
    /// * `code` - A string slice containing the registration code.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// - `Ok(Client)`: A new `Client` instance if registration and token creation are successful.
    /// - `Err(Error)`: An error if registration fails or creating the client from the token fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The registration process with the reMarkable Cloud fails.
    /// - Creating a new `Client` from the obtained token fails.
    pub async fn new(code: &str) -> Result<Client, Error> {
        log::debug!(
            "Registering client with reMarkable Cloud using code: {:?}",
            code
        );
        let auth_token = endpoints::register_client(code).await?;
        Ok(Client::from_token(&auth_token).await?)
    }

    /// Refreshes the authentication token for the client.
    ///
    /// This method performs the following steps:
    /// 1. Requests a new token from the server using the current token.
    /// 2. Updates the client's auth_token with the new token.
    /// 3. Saves the new token to the auth_token_file.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// - `Ok(())`: If the token refresh and saving are successful.
    /// - `Err(Error)`: If any step in the refresh process fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The token refresh request to the server fails.
    /// - Writing the new token to the auth_token_file fails.
    pub async fn refresh_token(&mut self) -> Result<(), Error> {
        log::debug!("Refreshing auth token");
        self.auth_token = endpoints::refresh_token(&self.auth_token).await?;
        log::debug!("New auth token: {:?}", self.auth_token);
        Ok(())
    }

    pub async fn sync_root(&self) -> Result<(), Error> {
        log::debug!("Getting items stored in the cloud");
        endpoints::sync_root(&self.storage_url, &self.auth_token).await?;
        Ok(())
    }

    pub async fn upload_file(&self, file: File) -> Result<(), Error> {
        log::debug!("Uploading a file to the cloud");
        endpoints::upload_file(&self.storage_url, &self.auth_token, file).await?;
        Ok(())
    }

    // pub async fn get_storage_url(&mut self) -> Result<String, Error> {
    //     let client = reqwest::Client::new();
    //     let response = client
    //         .get(formatcp!("{STORAGE_API_URL}/"))
    //         .header("Authorization", formatcp!("Bearer {self.token}"))
    //         .header("Content-Type", "application/json")
    //         .header("Accept", "application/json")
    //         .header("User-Agent", "google-chrome")
    //         .send()
    //         .await?;

    //     let storage_url: String = response.json().await?;
    //     self.storage_url = Some(storage_url);
    //     return Ok(storage_url);
    // }

    // pub async fn get_collection(&self, id: Uuid) -> Result<Collection, Error> {
    //     let client = reqwest::Client::new();
    //     let response = client
    //         .get(formatcp!("{STORAGE_API_URL}/{id}"))
    //         .header("Authorization", formatcp!("Bearer {self.token}"))
    //         .send()
    //         .await?;

    //     let collection: Collection = response.json().await?;
    //     return Ok(collection);
    // }

    // pub async fn get_document(&self, id: Uuid) -> Result<Document, Error> {
    //     let client = reqwest::Client::new();
    //     let response = client
    //         .get(formatcp!("{STORAGE_API_URL}/{id}"))
    //         .header("Authorization", formatcp!("Bearer {self.token}"))
    //         .send()
    //         .await?;

    //     let document: Document = response.json().await?;
    pub async fn list_files(&self) -> Result<Vec<crate::objects::Document>, Error> {
        log::debug!("Listing files in the cloud");
        endpoints::get_files(&self.storage_url, &self.auth_token).await
    }
}
