use std::collections::HashMap;
use std::env::current_dir;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use log::{debug, error};
use omnisette::adi_proxy::{
    ADIProxy, ADIProxyAnisetteProvider, Identifier, StartProvisioningData, STORE_DS_ID,
};
use omnisette::anisette_headers_provider::AnisetteHeadersProvider;
use omnisette::store_services_core::StoreServicesCoreADIProxy;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// An ADI provider instance, fixed to the App Store AMD directory-services id ([`STORE_DS_ID`] = -1).
/// This server serves only the App Store flow, so there is no ordinary (-2) / V1 path here.
pub struct ProviderInstance {
    provider: ADIProxyAnisetteProvider<StoreServicesCoreADIProxy<'static>>,
    path: PathBuf,
}

impl ProviderInstance {
    /// `adi_pb` is `Some` when continuing from an already-provisioned identity (get_headers), or
    /// `None` when a provisioning session is about to create one.
    pub fn new(identifier: Identifier, adi_pb: Option<Vec<u8>>) -> Result<ProviderInstance> {
        debug!("Creating a new ProviderInstance (ds_id={STORE_DS_ID})");
        let cwd = current_dir()?;

        let uuid = Uuid::from_bytes(identifier);
        let path = cwd.join("provisioning").join(uuid.to_string());
        if std::fs::create_dir_all(&path).is_ok() {}

        if let Some(adi_pb) = adi_pb {
            let adi_pb_path = path.join("adi.pb");
            if let Err(e) = std::fs::write(&adi_pb_path, adi_pb) {
                error!("Got error writing adi.pb (writing to {adi_pb_path:?}): {e:?}");
                return Err(anyhow!("Couldn't write adi.pb. We don't give you the exact error message to ensure nothing sensitive is shown, so please contact the server owner with the exact time this happened!"));
            }
        }

        let mut adi_proxy = StoreServicesCoreADIProxy::with_custom_provisioning_path(&cwd, &path)?;

        adi_proxy.set_device_identifier(uuid.to_string().to_uppercase())?; // UUID, uppercase

        let mut local_user_uuid_hasher = Sha256::new();
        local_user_uuid_hasher.update(identifier);
        adi_proxy
            .set_local_user_uuid(hex::encode(local_user_uuid_hasher.finalize()).to_uppercase()); // 64 uppercase character hex

        let mut provider = ADIProxyAnisetteProvider::without_identifier(adi_proxy).unwrap(); // this will never fail
        provider.set_ds_id(STORE_DS_ID); // request_otp / is_machine_provisioned use the App Store ds_id

        debug!("Successfully created a ProviderInstance");
        Ok(ProviderInstance { provider, path })
    }

    pub async fn get_authentication_headers_v3(&mut self) -> Result<HashMap<String, String>> {
        let headers = self.provider.get_anisette_headers(true).await?;

        let mut headers = self.provider.normalize_headers(headers);

        headers.remove("X-Mme-Client-Info"); // SideStore will get this using /client_info endpoint

        // SideStore will provide these properties
        // Even if it gets a wrong device ID and MD LU for the identifier (which it shouldn't, I've tested it a lot and it always gave the same as Rust)
        // we want it to use the same thing as what it used to send requests to apple servers
        headers.remove("X-Mme-Device-Id");
        headers.remove("X-Apple-I-MD-LU");
        headers.remove("X-Apple-I-SRL-NO");

        Ok(headers)
    }

    pub fn start_provisioning(&mut self, spim: &[u8]) -> Result<StartProvisioningData> {
        self.provider
            .adi_proxy()
            .start_provisioning(STORE_DS_ID, spim)
            .map_err(|e| anyhow!(e))
    }

    pub fn end_provisioning(&mut self, session: u32, ptm: &[u8], tk: &[u8]) -> Result<Vec<u8>> {
        self.provider
            .adi_proxy()
            .end_provisioning(session, ptm, tk)
            .map_err(|e| anyhow!(e))?;

        let adi_pb_path = self.path.join("adi.pb");
        match std::fs::read(adi_pb_path) {
            Ok(d) => Ok(d),
            Err(e) => {
                error!("Got error reading adi.pb: {e:?}");
                Err(anyhow!("Couldn't read adi.pb. We don't give you the exact error message to ensure nothing sensitive is shown, so please contact the server owner with the exact time this happened!"))
            }
        }
    }

    pub fn destroy_provisioning_session(&mut self, session: u32) -> Result<()> {
        self.provider
            .adi_proxy()
            .destroy_provisioning_session(session)
            .map_err(|e| anyhow!(e))
    }
}

impl Drop for ProviderInstance {
    fn drop(&mut self) {
        // Each request provisions into its own per-identifier directory; always clean it up.
        match std::fs::remove_dir_all(&self.path) {
            Ok(_) => debug!("Successfully cleaned up {:?}", self.path),
            Err(e) => error!(
                "Got error removing folder, server hoster might have to manually cleanup (removing {:?}): {e:?}",
                self.path
            ),
        };
    }
}
