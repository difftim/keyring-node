use napi_derive::napi;

#[napi(object)]
/// Options for creating an `Entry` or `AsyncEntry`.
///
/// All options are platform-specific: they are accepted on every platform but
/// only take effect where documented. Leaving an option absent keeps the
/// current default behavior.
pub struct EntryOptions {
  /// Linux-only options; ignored on other platforms.
  pub linux: Option<LinuxEntryOptions>,
  /// macOS-only options; ignored on other platforms.
  pub macos: Option<MacosEntryOptions>,
}

#[napi(object)]
/// Linux-only entry options; ignored on other platforms.
pub struct LinuxEntryOptions {
  /// Require a specific Linux credential store. When absent, the default
  /// auto-fallback selection is used (Secret Service, falling back to the
  /// kernel keyring). Requiring a store that is unavailable throws instead of
  /// falling back.
  pub store: Option<LinuxStore>,
}

#[napi(object)]
/// macOS-only entry options; ignored on other platforms.
pub struct MacosEntryOptions {
  /// Require a specific macOS credential store. When absent, the default
  /// file-based keychain is used. There is no fallback between stores.
  pub store: Option<MacosStore>,
  /// Accessibility of credentials created by this entry (`kSecAttrAccessible`
  /// policies, plus the `require-user-presence` access-control policy). Only
  /// supported together with `store:
  /// "protected"`; the file-based keychain has no per-item accessibility, so
  /// combining this with the `keychain` store (explicitly or by leaving
  /// `store` absent) throws. When absent, the protected store uses the
  /// system default, `when-unlocked`.
  pub access_policy: Option<MacosAccessPolicy>,
}

#[napi(string_enum)]
/// A macOS credential store that entries can be pinned to.
///
/// macOS only; ignored on other platforms. There is no fallback between
/// stores. Note that a missing entitlement does not fail construction: when
/// the process lacks the required code-signing entitlement, the `protected`
/// store is still constructed and the error surfaces on the first operation.
/// Construction itself can still throw, e.g. for `withTarget` (not supported
/// with the `protected` store) or an empty service or username.
pub enum MacosStore {
  /// The legacy file-based keychain. This is the default store.
  #[napi(value = "keychain")]
  Keychain,
  /// The data-protection keychain (`kSecUseDataProtectionKeychain`). Items
  /// are protected by the device passcode / Secure Enclave policies. The
  /// calling process must be code-signed with an application identifier;
  /// unsigned processes get an entitlement error. `target` is not supported
  /// with this store.
  #[napi(value = "protected")]
  Protected,
}

/// Define `MacosAccessPolicy` from one variant→string table, so the string a
/// JS caller passes and the string handed to the apple-native-keyring-store
/// `access-policy` modifier can never drift apart.
macro_rules! macos_access_policies {
  ($($(#[doc = $doc:literal])* $variant:ident => $value:literal,)*) => {
    #[napi(string_enum)]
    /// Accessibility of a credential in the macOS data-protection keychain,
    /// ordered from least to most restrictive.
    ///
    /// macOS only, and only supported with the `protected` store; ignored on
    /// other platforms. The `-this-device-only` variants additionally prevent
    /// the item from being migrated to another device (it still appears in
    /// encrypted backups, but cannot be restored onto different hardware).
    pub enum MacosAccessPolicy {
      $($(#[doc = $doc])* #[napi(value = $value)] $variant,)*
    }

    #[cfg(target_os = "macos")]
    impl MacosAccessPolicy {
      /// The value the apple-native-keyring-store `access-policy` modifier
      /// expects for this policy: identical to the string the JS caller
      /// passed, by construction.
      pub(crate) fn as_modifier_value(&self) -> &'static str {
        match self {
          $(MacosAccessPolicy::$variant => $value,)*
        }
      }
    }
  };
}

macos_access_policies! {
  /// Readable from the first unlock after boot until restart, even while the
  /// device is locked (`kSecAttrAccessibleAfterFirstUnlock`).
  AfterFirstUnlock => "after-first-unlock",
  /// Like `after-first-unlock`, but never migrated to another device.
  AfterFirstUnlockThisDeviceOnly => "after-first-unlock-this-device-only",
  /// Readable only while the device is unlocked
  /// (`kSecAttrAccessibleWhenUnlocked`). This is the system default.
  WhenUnlocked => "when-unlocked",
  /// Like `when-unlocked`, but never migrated to another device.
  WhenUnlockedThisDeviceOnly => "when-unlocked-this-device-only",
  /// Readable only while unlocked, only on devices with a passcode set, and
  /// deleted when the passcode is removed; never migrated.
  WhenPasscodeSetThisDeviceOnly => "when-passcode-set-this-device-only",
  /// Not a `kSecAttrAccessible` constant but an access-control policy: like
  /// `when-unlocked`, but every access additionally requires user presence
  /// (biometry or the device password), so reads prompt the user and can
  /// block or fail in headless or background processes.
  RequireUserPresence => "require-user-presence",
}

#[napi(string_enum)]
/// A Linux credential store that entries can be pinned to.
///
/// Linux only; ignored on other platforms. Requiring a store that is
/// unavailable on this machine throws instead of falling back.
pub enum LinuxStore {
  /// The freedesktop Secret Service (D-Bus) as provided by gnome-keyring or
  /// KWallet. Persistent daemon-backed storage.
  #[napi(value = "secret-service")]
  SecretService,
  /// The Linux kernel keyring via keyutils. In-memory only: credentials
  /// vanish on reboot.
  #[napi(value = "keyutils")]
  Keyutils,
}
