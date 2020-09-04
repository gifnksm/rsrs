use crate::{prelude::*, Result};
use futures_util::future::BoxFuture;
use std::{
    borrow::Cow,
    env,
    path::{Path, PathBuf},
    process,
};

mod daemon;
mod login;
mod open;
mod remote;

#[derive(Debug, clap::Clap)]
#[clap(name = clap::crate_name!(), version = clap::crate_version!(), author = clap::crate_authors!(), about = clap::crate_description!())]
pub(super) struct Opts {
    #[clap(flatten)]
    global: GlobalOpts,
    #[clap(subcommand)]
    sub_command: SubCommand,
}

impl Opts {
    pub(super) fn log_directive(&self) -> Option<&str> {
        self.global.log_directive.as_deref()
    }
}

#[derive(Debug, clap::Clap)]
struct GlobalOpts {
    /// Socket path
    #[clap(name = "sock-path", long)]
    sock_path: Option<PathBuf>,
    /// Log directive
    #[clap(name = "log", long)]
    log_directive: Option<String>,
}

impl GlobalOpts {
    fn sock_path(&self, is_leaf_daemon: bool) -> Cow<Path> {
        if let Some(path) = &self.sock_path {
            return path.as_path().into();
        }
        let mut tmp = env::temp_dir();
        if is_leaf_daemon {
            let pid = process::id();
            tmp.push(format!("rsrs.{}.sock", pid));
        } else {
            tmp.push("rsrs.root.sock");
        }
        tmp.into()
    }
}

#[derive(Debug, clap::Clap)]
enum SubCommand {
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Open(open::Opts),
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Login(login::Opts),
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Remote(remote::Opts),
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Daemon(daemon::Opts),
}

pub(crate) fn run(opts: Opts) -> BoxFuture<'static, Result<()>> {
    match opts.sub_command {
        SubCommand::Login(local) => login::run(opts.global, local).boxed(),
        SubCommand::Remote(local) => remote::run(opts.global, local).boxed(),
        SubCommand::Open(local) => open::run(opts.global, local).boxed(),
        SubCommand::Daemon(local) => daemon::run(opts.global, local).boxed(),
    }
}
