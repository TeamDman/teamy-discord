use crate::cli::attachment::list::AttachmentListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live attachment queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct AttachmentArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The attachment subcommand to run.
    #[facet(args::subcommand)]
    pub command: AttachmentCommand,
}

/// Attachment subcommands.
// cli[impl command.surface.live-attachment]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum AttachmentCommand {
    /// List attachments visible on the fetched messages.
    List(AttachmentListArgs),
}

impl AttachmentArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            AttachmentCommand::List(args) => args.invoke(&context).await,
        }
    }
}
