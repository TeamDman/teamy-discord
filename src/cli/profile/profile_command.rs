use super::clear::ProfileClearArgs;
use super::list::ProfileListArgs;
use super::set::ProfileSetArgs;
use super::show::ProfileShowArgs;
use super::use_profile::ProfileUseArgs;
use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ProfileCommand {
    /// Manage nonsecret per-profile broker account/item addresses.
    Broker(super::broker::ProfileBrokerArgs),
    /// Unset the active profile without removing named profiles.
    Clear(ProfileClearArgs),
    /// List saved profiles and the active name.
    List(ProfileListArgs),
    /// Create or replace a profile; omitted channel clears its channel default.
    Set(ProfileSetArgs),
    /// Show a named profile or the active profile.
    Show(ProfileShowArgs),
    /// Select the active profile.
    #[facet(rename = "use")]
    UseProfile(ProfileUseArgs),
}
