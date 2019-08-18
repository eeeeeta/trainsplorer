//! Handles interacting with Google Cloud Storage.

pub mod errors;

use yup_oauth2::ServiceAccountAccess;
use google_storage1::Storage;
use hyper::{Client};
use hyper::net::HttpsConnector;
use hyper_rustls::TlsClient;
use std::fs::File;

pub use google_storage1::{Object};

use crate::errors::*;

/// Google Cloud Storage client.
pub struct CloudStorage {
    inner: Storage<Client, ServiceAccountAccess<Client>>,
    bucket_name: String
}

impl CloudStorage {
    /// Set up this GCS client to access a named bucket (`bucket_name`).
    ///
    /// `key_path` (unfortunately a `String` due to stupid oauth crate brain
    /// damages) is a filesystem path to a service account key file, in JSON
    /// format.
    pub fn init(bucket_name: String, key_path: &String) -> GcsResult<Self> {
        let key = yup_oauth2::service_account_key_from_file(key_path)?;
        let cli = Client::with_connector(HttpsConnector::new(TlsClient::new()));
        let cli2 = Client::with_connector(HttpsConnector::new(TlsClient::new()));
        let sa = ServiceAccountAccess::new(key, cli2);
        let inner = Storage::new(cli, sa);
        Ok(Self { inner, bucket_name })
    }
    /// Get metadata about an object in the bucket, under the path `obj`.
    pub fn get_object(&mut self, obj: &str) -> GcsResult<Object> {
        let ret = self.inner.objects()
            .get(&self.bucket_name, obj)
            .doit()?;
        Ok(ret.1)
    }
    /// Download the object at the path `obj` to the filesystem path `to_path`.
    pub fn download_object(&mut self, obj: &str, to_path: &str) -> GcsResult<()> {
        let mut file = File::create(to_path)?;
        let mut ret = self.inner.objects()
            .get(&self.bucket_name, obj)
            .param("alt", "media")
            .doit()?;
        std::io::copy(&mut ret.0, &mut file)?;
        Ok(())
    }
    /// Upload the object at the filesystem path `from_path` to the object path `obj`.
    pub fn upload_object(&mut self, from_path: &str, obj: &str) -> GcsResult<()> {
        let mut file = File::open(from_path)?;
        let mut object = Object::default();
        object.name = Some(obj.into());
        self.inner.objects()
            .insert(object, &self.bucket_name)
            .upload_resumable(&mut file, "application/octet-stream".parse().unwrap())?;;
        Ok(())
    }
}

