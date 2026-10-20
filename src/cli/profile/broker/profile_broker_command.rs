use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ProfileBrokerCommand {
    /// Set nonsecret account/reference fields; omitted fields are preserved.
    Set(super::set::ProfileBrokerSetArgs),
    /// Show saved nonsecret broker fields for a named or active profile.
    Show(super::show::ProfileBrokerShowArgs),
    /// Clear selected nonsecret broker fields; does not purge leases or delete tokens.
    Clear(super::clear::ProfileBrokerClearArgs),
}
