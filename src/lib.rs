#![warn(missing_docs)]
#![forbid(unsafe_code)]

//! Interns str slices, giving you back an ID value.
//!
//! The ID value is a newtyped [NonZeroUsize]. You get approximately the same
//! benefits as using `Box<Str>`, but when the same str is used in multiple
//! locations they'll resolve to the same ID value.
//!
//! All str slice data resides in a global cache. There is no way to purge the
//! cache once a str slice has been interned. This library is not intended for
//! long running programs.

use bimap::BiHashMap;
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
};
use std::sync::{OnceLock, PoisonError, RwLock};

/// An easier name to type because you don't have to use non-letter characters.
pub type StaticStr = &'static str;

#[cfg(not(feature = "fnv"))]
type BiMap = BiHashMap<StrID, StaticStr>;
#[cfg(feature = "fnv")]
type BiMap =
  BiHashMap<StrID, StaticStr, fnv::FnvBuildHasher, fnv::FnvBuildHasher>;

static NEXT_STR_ID: AtomicUsize = AtomicUsize::new(1);

static STR_CACHE: OnceLock<RwLock<BiMap>> = OnceLock::new();

/// This is a newtype over a [NonZeroUsize] which can get back the str slice
/// used to obtain this ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct StrID(NonZeroUsize);
impl StrID {
  #[inline]
  fn try_new() -> Option<Self> {
    NonZeroUsize::new(NEXT_STR_ID.fetch_add(1, Ordering::Relaxed)).map(Self)
  }

  #[inline]
  #[track_caller]
  fn new() -> Self {
    Self::try_new().expect("exhausted the available StrID values!")
  }

  /// Unwraps the value into a raw `usize`.
  #[inline]
  #[must_use]
  pub const fn as_usize(self) -> usize {
    self.0.get()
  }

  /// Gets the str slice associated with this ID value.
  #[inline]
  #[must_use]
  pub fn as_str(self) -> StaticStr {
    let rw_lock = STR_CACHE.get_or_init(|| RwLock::new(BiMap::default()));
    let read = rw_lock.read().unwrap_or_else(PoisonError::into_inner);
    read.get_by_left(&self).unwrap_or(&"")
  }
}

impl core::fmt::Debug for StrID {
  #[inline]
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Debug::fmt(&self.as_str(), f)
  }
}

impl core::fmt::Display for StrID {
  #[inline]
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Display::fmt(&self.as_str(), f)
  }
}

impl<'a> From<Box<str>> for StrID {
  #[inline]
  fn from(value: Box<str>) -> Self {
    let s: &str = &*value;
    let rw_lock = STR_CACHE.get_or_init(|| RwLock::new(BiMap::default()));
    let read = rw_lock.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(id) = read.get_by_right(s) {
      *id
    } else {
      drop(read);
      let mut write = rw_lock.write().unwrap_or_else(PoisonError::into_inner);
      // It's *possible* that the string was inserted after we dropped the
      // reader before we acquired the writer, so we must check again.
      if let Some(id) = write.get_by_right(s) {
        *id
      } else {
        let id: StrID = StrID::new();
        let leaked: StaticStr = Box::leak(value);
        write.insert(id, leaked);
        id
      }
    }
  }
}

impl<'a> From<&'a str> for StrID {
  #[inline]
  fn from(s: &'a str) -> Self {
    // essentially the same as the `Box<str>` version, just that we have to box
    // the data if it does have to be inserted into the cache.
    let rw_lock = STR_CACHE.get_or_init(|| RwLock::new(BiMap::default()));
    let read = rw_lock.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(id) = read.get_by_right(&s) {
      *id
    } else {
      drop(read);
      let mut write = rw_lock.write().unwrap_or_else(PoisonError::into_inner);
      if let Some(id) = write.get_by_right(s) {
        *id
      } else {
        let id: StrID = StrID::new();
        let leaked: StaticStr = Box::leak(s.to_string().into_boxed_str());
        write.insert(id, leaked);
        id
      }
    }
  }
}

impl From<String> for StrID {
  #[inline]
  fn from(s: String) -> Self {
    // essentially the same as the `Box<str>` version, just that we have to
    // convert String into Box<str> the data if it does have to be inserted into
    // the cache (which might be free or it might be a reallocation).
    let rw_lock = STR_CACHE.get_or_init(|| RwLock::new(BiMap::default()));
    let read = rw_lock.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(id) = read.get_by_right(s.as_str()) {
      *id
    } else {
      drop(read);
      let mut write = rw_lock.write().unwrap_or_else(PoisonError::into_inner);
      if let Some(id) = write.get_by_right(s.as_str()) {
        *id
      } else {
        let id: StrID = StrID::new();
        let leaked: StaticStr = Box::leak(s.into_boxed_str());
        write.insert(id, leaked);
        id
      }
    }
  }
}

impl AsRef<str> for StrID {
  #[inline]
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl Default for StrID {
  #[inline]
  fn default() -> Self {
    Self::from(<&str>::default())
  }
}
