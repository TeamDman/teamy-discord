use eyre::Context;
use std::io::Write;

/// Write one complete JSON result and check both writing and flushing.
/// # Errors
/// Propagates serialization, write and flush failures without reporting completed output.
pub fn render_response<'facet, T: facet::Facet<'facet> + ?Sized>(
    writer: &mut impl Write,
    response: &T,
) -> eyre::Result<()> {
    let encoded = facet_json::to_string(response).wrap_err("Failed to serialize watcher result")?;
    writeln!(writer, "{encoded}").wrap_err("Failed to write watcher result")?;
    writer.flush().wrap_err("Failed to flush watcher result")
}
