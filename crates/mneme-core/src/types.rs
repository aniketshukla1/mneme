//! Core identifiers, scoping, and the bi-temporal time model.
//!
//! See architecture report §5 (Data Model). Every memory and every edge
//! carries a [`BiTemporal`] stamp: we never overwrite history, we invalidate
//! and create a new version. This is what makes evolution + audit coexist.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

/// Monotonic, sortable identifier used for all log entries and entities.
pub type Id = Ulid;

/// Generates a fresh time-ordered id.
pub fn new_id() -> Id {
    Ulid::new()
}

/// Bi-temporal stamp: validity time (when the fact was true in the world)
/// and transaction time (when the system learned/forgot it).
///
/// `valid_to` / `tx_to` are `None` while the fact is current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiTemporal {
    pub valid_from: OffsetDateTime,
    pub valid_to: Option<OffsetDateTime>,
    pub tx_from: OffsetDateTime,
    pub tx_to: Option<OffsetDateTime>,
}

impl BiTemporal {
    /// A stamp that is valid-now and was just recorded.
    pub fn now() -> Self {
        let t = OffsetDateTime::now_utc();
        Self { valid_from: t, valid_to: None, tx_from: t, tx_to: None }
    }

    /// True if the fact is valid at `at` and not yet superseded in the system.
    pub fn is_live(&self, at: OffsetDateTime) -> bool {
        let valid = self.valid_from <= at && self.valid_to.map_or(true, |e| at < e);
        let current = self.tx_to.is_none();
        valid && current
    }
}

/// Tenancy + ownership boundary. Procedural learning and memory evolution
/// must never cross a scope boundary without an explicit aggregation step
/// (see report §10, "Privacy").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope {
    pub tenant: String,
    pub user: Option<String>,
    /// `None` => applies to the whole user/tenant; `Some` => single session.
    pub session: Option<String>,
}

impl Scope {
    pub fn global(tenant: impl Into<String>) -> Self {
        Self { tenant: tenant.into(), user: None, session: None }
    }

    /// True if `self` is allowed to read/learn from data stamped `other`.
    /// Global scope contains user scope contains session scope.
    pub fn contains(&self, other: &Scope) -> bool {
        if self.tenant != other.tenant {
            return false;
        }
        match (&self.user, &other.user) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(a), Some(b)) => {
                a == b
                    && match (&self.session, &other.session) {
                        (None, _) => true,
                        (Some(_), None) => false,
                        (Some(x), Some(y)) => x == y,
                    }
            }
        }
    }
}

/// Typed references — newtypes so the compiler stops us mixing id kinds.
macro_rules! id_ref {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Id);
        impl From<Id> for $name {
            fn from(id: Id) -> Self {
                Self(id)
            }
        }
    };
}

id_ref!(MemoryRef);
id_ref!(EpisodeRef);
id_ref!(OutcomeRef);
id_ref!(ArtifactRef);
id_ref!(TrajectoryRef);
id_ref!(ProposalId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_containment() {
        let g = Scope::global("acme");
        let u = Scope { tenant: "acme".into(), user: Some("aniket".into()), session: None };
        let s = Scope {
            tenant: "acme".into(),
            user: Some("aniket".into()),
            session: Some("sess-1".into()),
        };
        assert!(g.contains(&u));
        assert!(g.contains(&s));
        assert!(u.contains(&s));
        assert!(!s.contains(&u)); // session cannot read user-wide
        assert!(!u.contains(&g)); // user cannot read global
    }
}
