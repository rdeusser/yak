/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(used_with_arg)]

use std::thread;

use clap::CommandFactory;
use clap::FromArgMatches;
use dupe::Dupe;
use yak_client::commands::build::BuildCommand;
use yak_client::commands::bxl::BxlCommand;
use yak_client::commands::clean::CleanCommand;
use yak_client::commands::cleanall::CleanallCommand;
use yak_client::commands::ctargets::ConfiguredTargetsCommand;
use yak_client::commands::expand_external_cell::ExpandExternalCellsCommand;
use yak_client::commands::help_env::HelpEnvCommand;
use yak_client::commands::init::InitCommand;
use yak_client::commands::install::InstallCommand;
use yak_client::commands::kill::KillCommand;
use yak_client::commands::killall::KillallCommand;
use yak_client::commands::lsp::LspCommand;
use yak_client::commands::profile::ProfileCommand;
use yak_client::commands::query::aquery::AqueryCommand;
use yak_client::commands::query::cquery::CqueryCommand;
use yak_client::commands::query::uquery::UqueryCommand;
use yak_client::commands::root::RootCommand;
use yak_client::commands::run::RunCommand;
use yak_client::commands::server::ServerCommand;
use yak_client::commands::status::StatusCommand;
use yak_client::commands::subscribe::SubscribeCommand;
use yak_client::commands::targets::TargetsCommand;
use yak_client::commands::test::TestCommand;
use yak_client_ctx::argfiles::expand_argv;
use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::client_ctx::YakSubcommand;
use yak_client_ctx::client_metadata::ClientMetadata;
use yak_client_ctx::client_metadata::parse_client_metadata;
use yak_client_ctx::common::YakArgMatches;
use yak_client_ctx::exit_result::ClientIoError;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::immediate_config::ImmediateConfigContext;
use yak_client_ctx::version::YakVersion;
use yak_cmd_audit_client::AuditCommand;
use yak_cmd_debug_client::DebugCommand;
use yak_cmd_log_client::LogCommand;
use yak_cmd_starlark_client::StarlarkCommand;
use yak_common::argv::Argv;
use yak_common::invocation_paths::RESERVED_YAK_OUT_PREFIX;
use yak_common::invocation_paths_result::InvocationPathsResult;
use yak_common::invocation_roots::get_invocation_paths_result;
use yak_common::settings::args::SettingOverride;
use yak_common::settings::args::parse_setting_flag_arg;
use yak_core::yak_env;
use yak_core::yak_env_name;
use yak_data::ErrorReport;
use yak_error::ErrorTag;
use yak_error::ExitCode;
use yak_error::YakErrorContext;
use yak_error::conversion::clap::yak_error_clap_parser;
use yak_event_observer::verbosity::Verbosity;
use yak_fs::paths::file_name::FileNameBuf;
use yak_util::threads::thread_spawn_scoped;

use crate::check_user_allowed::check_user_allowed;
use crate::process_context::ProcessContext;

mod check_user_allowed;
mod cli_style;
pub(crate) mod commands;
pub mod process_context;
pub mod soft_error;

fn parse_isolation_dir(s: &str) -> yak_error::Result<FileNameBuf> {
    if s.starts_with(RESERVED_YAK_OUT_PREFIX) {
        return Err(yak_error::yak_error!(
            yak_error::ErrorTag::Input,
            "Isolation dir names starting with `{RESERVED_YAK_OUT_PREFIX}` are reserved for yak's internal use"
        ));
    }
    FileNameBuf::try_from(s.to_owned()).yak_error_context("isolation dir must be a directory name")
}

/// Options of `yak` command, before subcommand.
#[derive(Clone, Debug, clap::Parser)]
#[clap(next_help_heading = "Universal Options")]
struct BeforeSubcommandOptions {
    /// The name of the directory that yak creates within yak-out for writing outputs and daemon
    /// information. If one is not provided, yak creates a directory with the default name.
    ///
    /// Instances of yak share a daemon if and only if their isolation directory is identical.
    /// The isolation directory also influences the output paths provided by yak,
    /// and as a result using a non-default isolation dir will cause cache misses (and slower builds).
    #[clap(
        value_parser = yak_error_clap_parser(parse_isolation_dir),
        env("YAK_ISOLATION_DIR"),
        long,
        global = true,
        default_value="v2"
    )]
    isolation_dir: FileNameBuf,

    /// How verbose yak should be while logging.
    ///
    /// Values:
    /// 0 = Quiet, errors only;
    /// 1 = Show status. Default;
    /// 2 = more info about errors;
    /// 3 = more info about everything;
    /// 4 = more info about everything + stderr;
    ///
    /// It can be combined with specific log items (stderr, full_failed_command, commands, actions,
    /// status, stats, success) to fine-tune the verbosity of the log. Example usage "-v=1,stderr"
    #[clap(
        short = 'v',
        long = "verbose",
        default_value = "1",
        global = true,
        env = yak_env_name!("YAK_VERBOSE"),
        value_parser = yak_error_clap_parser(Verbosity::try_from_cli)
    )]
    verbosity: Verbosity,

    /// The team that owns this command. yak records it in the event log.
    #[clap(long, global = true)]
    oncall: Option<String>,

    /// Metadata key-value pairs to record in the event log. Client metadata must be of the form
    /// `key=value`, where `key` is a snake_case identifier.
    #[clap(long, global = true, value_parser = yak_error_clap_parser(parse_client_metadata))]
    client_metadata: Vec<ClientMetadata>,

    /// Override a yak setting using `section.key=value`.
    #[clap(
        long = "setting",
        value_name = "SECTION.KEY=VALUE",
        global = true,
        num_args = 1,
        value_parser = yak_error_clap_parser(parse_setting_flag_arg)
    )]
    settings: Vec<SettingOverride>,

    /// Do not launch a daemon process, run yak server in client process.
    ///
    /// Note even when running in no-yakd mode, it still writes state files.
    /// In particular, this command effectively kills yakd process
    /// running with the same isolation directory.
    ///
    /// This is an unsupported option used only for development work.
    #[clap(env("YAK_NO_YAKD"), long = "no-yakd", global(true), hide(true))]
    no_yakd: bool,
}

fn help() -> &'static str {
    concat!(
        "A build system\n",
        "\n",
        "Documentation: https://rdeusser.github.io/yak/docs/\n",
    )
}

#[derive(Debug, clap::Parser)]
#[clap(
    name = "yak",
    about(Some(help())),
    version(YakVersion::get_version_for_clap()),
    styles = cli_style::get_styles(),
)]
pub(crate) struct Opt {
    #[clap(subcommand)]
    cmd: CommandKind,
    #[clap(flatten)]
    common_opts: BeforeSubcommandOptions,
}

impl Opt {
    pub(crate) fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: YakArgMatches<'_>,
        argv: Argv,
    ) -> ExitResult {
        let subcommand_matches = matches.unwrap_subcommand();

        self.cmd.exec(
            process,
            immediate_config,
            subcommand_matches,
            argv,
            self.common_opts,
        )
    }
}

pub fn exec(process: ProcessContext<'_>) -> ExitResult {
    let cwd = process.shared.working_dir.clone();
    let mut immediate_config = ImmediateConfigContext::new(&cwd);
    let arg0_override = yak_env!("YAK_ARG0")?;
    let expanded_args = expand_argv(
        arg0_override,
        process.shared.args.to_vec(),
        &mut immediate_config,
        &cwd,
    )
    .yak_error_context("Error expanding argsfiles")?;

    let argv = Argv {
        argv: process.shared.args.to_vec(),
        expanded_argv: expanded_args,
    };

    let clap = Opt::command();
    let matches = match clap.try_get_matches_from(argv.expanded_argv.args()) {
        Ok(matches) => matches,
        Err(e) => {
            // Print colorized output, ExitResult::report will not colorize.
            // `ClientIoError` so that a closed stdout exits quietly instead of as a yak failure,
            // e.g. `yak build --help | head`.
            e.print().map_err(ClientIoError::from)?;
            return if e.exit_code() == 0 {
                ExitResult::success()
            } else {
                let e = yak_error::Error::from(e).tag([ErrorTag::ClapMatch]);
                ExitResult::status_with_emitted_errors(
                    ExitCode::UserError,
                    vec![ErrorReport::from(&e)],
                )
            };
        }
    };
    let mut opt = ParsedArgv::parse(argv, matches)?;
    let setting_arg_layers = opt
        .opt
        .common_opts
        .settings
        .drain(..)
        .map(SettingOverride::into_table)
        .collect();
    immediate_config.set_setting_arg_layers(setting_arg_layers)?;

    let client_metadata = ClientMetadata::from_env()?;
    if !client_metadata.is_empty() {
        // insert the `client_metadata` at the beginning of the list, so that the client id metadata from the env var could be overridden by the cli arg
        opt.opt
            .common_opts
            .client_metadata
            .splice(0..0, client_metadata);
    }
    opt.exec(process, &immediate_config)
}

struct ParsedArgv {
    opt: Opt,
    argv: Argv,
    matches: clap::ArgMatches,
}

impl ParsedArgv {
    fn parse(argv: Argv, matches: clap::ArgMatches) -> yak_error::Result<Self> {
        let opt: Opt = Opt::from_arg_matches(&matches)?;

        match &opt.cmd {
            #[cfg(not(client_only))]
            CommandKind::Daemon(..) | CommandKind::Forkserver(..) => {}
            CommandKind::Clean(..) | CommandKind::Cleanall(..) => {}
            _ => {
                check_user_allowed()?;
            }
        }

        Ok(ParsedArgv { opt, argv, matches })
    }

    fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
    ) -> ExitResult {
        let expanded_args = self.argv.expanded_argv.clone();
        self.opt.exec(
            process,
            immediate_config,
            YakArgMatches::from_clap(&self.matches, &expanded_args),
            self.argv,
        )
    }
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum CommandKind {
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    Daemon(yak_daemon::daemon::DaemonCommand),
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    Forkserver(crate::commands::forkserver::ForkserverCommand),
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    InternalTestRunner(crate::commands::internal_test_runner::InternalTestRunnerCommand),
    #[clap(subcommand)]
    Audit(AuditCommand),
    Aquery(AqueryCommand),
    Build(BuildCommand),
    Bxl(BxlCommand),
    // TODO(nga): implement `yak help-yakconfig` too
    HelpEnv(HelpEnvCommand),
    Test(TestCommand),
    Cquery(CqueryCommand),
    Init(InitCommand),
    ExpandExternalCell(ExpandExternalCellsCommand),
    Install(InstallCommand),
    Kill(KillCommand),
    Killall(KillallCommand),
    Root(RootCommand),
    /// Alias for `uquery`.
    Query(UqueryCommand),
    Run(RunCommand),
    Server(ServerCommand),
    Status(StatusCommand),
    #[clap(subcommand)]
    Starlark(StarlarkCommand),
    /// Alias for `utargets`.
    Targets(TargetsCommand),
    Utargets(TargetsCommand),
    Ctargets(ConfiguredTargetsCommand),
    Uquery(UqueryCommand),
    #[clap(subcommand, hide = true)]
    Debug(DebugCommand),
    #[clap(hide = true)]
    Complete(yak_cmd_completion_client::complete::CompleteCommand),
    Completion(yak_cmd_completion_client::completion::CompletionCommand),
    Docs(yak_cmd_docs_client::DocsCommand),
    #[clap(subcommand)]
    Profile(ProfileCommand),
    Clean(CleanCommand),
    Cleanall(CleanallCommand),
    #[clap(subcommand)]
    Log(LogCommand),
    Lsp(LspCommand),
    Subscribe(SubscribeCommand),
}

impl CommandKind {
    pub(crate) fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: YakArgMatches<'_>,
        argv: Argv,
        common_opts: BeforeSubcommandOptions,
    ) -> ExitResult {
        let paths_result = get_invocation_paths_result(
            &process.shared.working_dir,
            common_opts.isolation_dir.clone(),
        );

        // Handle the daemon command earlier: it wants to fork, but the things we do below might
        // want to create threads.
        #[cfg(not(client_only))]
        if let CommandKind::Daemon(cmd) = self {
            process.events_ctx.log_invocation_record = false;
            return cmd
                .exec(
                    process.shared.log_reload_handle.dupe(),
                    paths_result.get_result()?,
                    false,
                    || {},
                )
                .into();
        }
        thread::scope(|scope| {
            // Spawn a thread to have stack size independent on linker/environment.
            match thread_spawn_scoped("yak-main", scope, move || {
                self.exec_no_daemon(
                    common_opts,
                    process,
                    immediate_config,
                    matches,
                    argv,
                    paths_result,
                )
            }) {
                Ok(t) => match t.join() {
                    Ok(res) => res,
                    Err(_) => ExitResult::bail("Main thread panicked"),
                },
                Err(e) => ExitResult::bail(format_args!("Failed to start main thread: {e}")),
            }
        })
    }

    fn exec_no_daemon(
        self,
        common_opts: BeforeSubcommandOptions,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: YakArgMatches<'_>,
        argv: Argv,
        paths: InvocationPathsResult,
    ) -> ExitResult {
        if common_opts.no_yakd {
            // `no_yakd` can't work in a client-only binary
            if let Some(res) = ExitResult::retry_command_with_full_binary()? {
                return res;
            }
        }

        let ProcessContext {
            trace_id,
            events_ctx,
            shared,
            runtime,
            start_time,
        } = process;

        let runtime = runtime.get_or_init()?;

        let start_in_process_daemon = if common_opts.no_yakd {
            #[cfg(not(client_only))]
            {
                yak_daemon::no_yakd::start_in_process_daemon(
                    immediate_config.daemon_startup_config()?,
                    paths.clone().get_result()?,
                    runtime,
                )?
            }
            #[cfg(client_only)]
            {
                unreachable!() // case covered above
            }
        } else {
            None
        };

        let command_ctx = ClientCommandContext::new(
            immediate_config,
            paths,
            shared.working_dir.clone(),
            common_opts.verbosity,
            start_time,
            start_in_process_daemon,
            argv,
            trace_id.dupe(),
            &mut shared.stdin,
            &mut shared.restarter,
            runtime,
            common_opts.oncall,
            common_opts.client_metadata,
            common_opts.isolation_dir,
        );
        if let Some(recorder) = events_ctx.recorder.as_mut() {
            recorder.update_for_client_ctx(&command_ctx, self.command_name());
        }

        match self {
            #[cfg(not(client_only))]
            CommandKind::Daemon(..) => unreachable!("Checked earlier"),
            #[cfg(not(client_only))]
            CommandKind::Forkserver(cmd) => cmd.exec(
                matches,
                command_ctx,
                events_ctx,
                shared.log_reload_handle.dupe(),
            ),
            #[cfg(not(client_only))]
            CommandKind::InternalTestRunner(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Aquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Build(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Bxl(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Test(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Cquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::HelpEnv(cmd) => cmd.exec(matches, command_ctx),
            CommandKind::Kill(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Killall(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Clean(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Cleanall(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Root(cmd) => cmd.exec(matches, command_ctx).into(),
            CommandKind::Query(cmd) => {
                yak_client_ctx::eprintln!(
                    "WARNING: \"yak query\" is an alias for \"yak uquery\". Consider using \"yak cquery\" or \"yak uquery\" explicitly."
                )?;
                command_ctx.exec(cmd, matches, events_ctx)
            }
            CommandKind::Server(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Status(cmd) => cmd.exec(matches, command_ctx).into(),
            CommandKind::Targets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Utargets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Ctargets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Audit(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Starlark(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Run(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Uquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Debug(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Complete(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Completion(cmd) => cmd.exec(Opt::command(), matches, command_ctx),
            CommandKind::Docs(cmd) => cmd.exec(Opt::command(), matches, command_ctx, events_ctx),
            CommandKind::Profile(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Init(cmd) => cmd.exec(matches, command_ctx),
            CommandKind::Install(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Log(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Lsp(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Subscribe(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::ExpandExternalCell(cmd) => command_ctx.exec(cmd, matches, events_ctx),
        }
    }

    fn command_name(&self) -> &'static str {
        match self {
            #[cfg(not(client_only))]
            CommandKind::Daemon(_) => "daemon",
            #[cfg(not(client_only))]
            CommandKind::Forkserver(_) => "forkserver",
            #[cfg(not(client_only))]
            CommandKind::InternalTestRunner(_) => "internal-test-runner",
            CommandKind::Aquery(cmd) => cmd.logging_name(),
            CommandKind::Build(cmd) => cmd.logging_name(),
            CommandKind::Bxl(cmd) => cmd.logging_name(),
            CommandKind::Test(cmd) => cmd.logging_name(),
            CommandKind::Cquery(cmd) => cmd.logging_name(),
            CommandKind::HelpEnv(_) => "help-env",
            CommandKind::Kill(cmd) => cmd.logging_name(),
            CommandKind::Killall(cmd) => cmd.logging_name(),
            CommandKind::Clean(cmd) => cmd.command_name(),
            CommandKind::Cleanall(cmd) => cmd.logging_name(),
            CommandKind::Root(_) => "root",
            CommandKind::Query(cmd) => cmd.logging_name(),
            CommandKind::Server(cmd) => cmd.logging_name(),
            CommandKind::Status(_) => "status",
            CommandKind::Targets(cmd) => cmd.logging_name(),
            CommandKind::Utargets(cmd) => cmd.logging_name(),
            CommandKind::Ctargets(cmd) => cmd.logging_name(),
            CommandKind::Audit(cmd) => cmd.logging_name(),
            CommandKind::Starlark(cmd) => cmd.command_name(),
            CommandKind::Run(cmd) => cmd.logging_name(),
            CommandKind::Uquery(cmd) => cmd.logging_name(),
            CommandKind::Debug(_) => "debug",
            CommandKind::Complete(_) => "complete",
            CommandKind::Completion(_) => "completion",
            CommandKind::Docs(_) => "docs",
            CommandKind::Profile(_) => "profile",
            CommandKind::Init(_) => "init",
            CommandKind::Install(cmd) => cmd.logging_name(),
            CommandKind::Log(cmd) => cmd.command_name(),
            CommandKind::Lsp(cmd) => cmd.logging_name(),
            CommandKind::Subscribe(cmd) => cmd.logging_name(),
            CommandKind::ExpandExternalCell(cmd) => cmd.logging_name(),
        }
    }
}
