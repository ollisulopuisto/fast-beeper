pub mod backend {
    pub mod beeper {
        #[derive(Clone)]
        pub struct BeeperConfig {
            pub base_url: String,
            pub token: String,
        }
        impl BeeperConfig {
            pub fn from_env() -> Option<Self> {
                None
            }
        }
    }
}
pub mod archive {
    pub mod encryption {
        pub fn platform_store()
        -> Result<std::sync::Arc<dyn keyring_core::api::CredentialStoreApi>, String> {
            zbus_secret_service_keyring_store::Store::new()
                .map(|s| s as std::sync::Arc<dyn keyring_core::api::CredentialStoreApi>)
                .map_err(|_| "keyring".into())
        }
    }
}
#[path = "../../../src/beeper_signin.rs"]
pub mod beeper_signin;
