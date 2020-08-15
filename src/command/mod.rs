use crate::Result;

mod login;
mod remote;

#[derive(Debug, clap::Clap)]
#[clap(name = clap::crate_name!(), version = clap::crate_version!(), author = clap::crate_authors!(), about = clap::crate_description!())]
pub(super) struct Opts {
    #[clap(flatten)]
    global: GlobalOpts,
    #[clap(subcommand)]
    sub_command: SubCommand,
}

#[derive(Debug, clap::Clap)]
struct GlobalOpts {}

#[derive(Debug, clap::Clap)]
enum SubCommand {
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Login(login::Opts),
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Remote(remote::Opts),
}

pub(crate) async fn run(opts: Opts) -> Result<()> {
    match opts.sub_command {
        SubCommand::Login(local) => login::run(opts.global, local).await?,
        SubCommand::Remote(local) => remote::run(opts.global, local).await?,
    }

    Ok(())
}
