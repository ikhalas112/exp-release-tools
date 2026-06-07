mod channel;
mod config;
mod inject;
mod manifest;
mod mock_build;
mod package;
mod protect;
mod r2_config;
mod resolve;
mod sync;
mod verify;
mod version_cmp;

use anyhow::Result;
use clap::{Parser, Subcommand};
use resolve::Platform;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "maxgame-release")]
#[command(about = "Windows single-exe release engine (protect + deploy to R2)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Resolve tag → env/channel/version/paths
    Resolve {
        tag: String,
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long, default_value = "env")]
        format: String,
        #[arg(long, value_enum, default_value_t = Platform::Windows)]
        platform: Platform,
    },
    /// Copy mock fixture → build/ (windows: game.exe + assets/, macos: .app bundle)
    MockBuild {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Platform::Windows)]
        platform: Platform,
    },
    /// Write version.txt into assets before protect
    InjectVersion {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        channel: String,
        #[arg(long)]
        date: Option<String>,
    },
    /// Encrypt + compress + embed assets into single .exe via maxion-protector
    Protect {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long, default_value = "build")]
        build_dir: PathBuf,
        #[arg(long, default_value = "output")]
        output_dir: PathBuf,
        #[arg(long)]
        stub_dll: Option<PathBuf>,
    },
    /// Pack macOS .app bundle into a single zip via ditto (no protect)
    Package {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long, default_value = "build")]
        build_dir: PathBuf,
        #[arg(long, default_value = "output")]
        output_dir: PathBuf,
        #[arg(long, value_enum, default_value_t = Platform::Macos)]
        platform: Platform,
    },
    /// Generate manifest.json + channel-manifest.json (single_exe)
    Manifest {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long, default_value = "output")]
        output_dir: PathBuf,
        #[arg(long, value_enum, default_value_t = Platform::Windows)]
        platform: Platform,
    },
    /// Upload output/ to R2 (multi-dest, parallel)
    Sync {
        local_dir: PathBuf,
        #[arg(long = "dest", required = true)]
        dest: Vec<String>,
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        /// Skip remote keys under these prefixes (relative to dest) from mirror delete
        #[arg(long = "exclude-prefix")]
        exclude_prefix: Vec<String>,
    },
    /// Update channels/{channel}/manifest.json with version guard
    UpdateChannel {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        local: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = Platform::Windows)]
        platform: Platform,
    },
    /// Post-deploy CDN verify (single artifact)
    Verify {
        #[arg(long, default_value = "release.config.json")]
        config: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long, value_enum, default_value_t = Platform::Windows)]
        platform: Platform,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Resolve {
            tag,
            config,
            format,
            platform,
        } => {
            let cfg = config::load_config(Some(&config))?;
            let r = resolve::resolve_release(&tag, &cfg, platform)?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&r)?),
                "env" => resolve::print_env(
                    &r,
                    &resolve::artifact_file(&cfg, platform),
                    &resolve::version_inject_dir(&cfg, platform),
                ),
                other => anyhow::bail!("unknown format: {other}"),
            }
        }
        Commands::MockBuild {
            config,
            fixture,
            output,
            platform,
        } => {
            let cfg = config::load_config(Some(&config))?;
            let output = output.unwrap_or_else(|| PathBuf::from(&cfg.build.dir));
            match platform {
                Platform::Windows => {
                    let fixture =
                        fixture.unwrap_or_else(|| PathBuf::from(&cfg.build.mock_fixture));
                    mock_build::mock_build(&fixture, &output)?;
                }
                Platform::Macos => {
                    let mac = cfg.macos.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("mock-build --platform=macos requires a \"macos\" block")
                    })?;
                    let fixture = fixture.unwrap_or_else(|| PathBuf::from(&mac.mock_fixture));
                    mock_build::mock_build_macos(&fixture, &output, &mac.app_bundle)?;
                }
            }
        }
        Commands::InjectVersion {
            source,
            tag,
            channel,
            date,
        } => {
            inject::inject_version(&source, &tag, &channel, date.as_deref())?;
        }
        Commands::Protect {
            config,
            tag: _,
            build_dir,
            output_dir,
            stub_dll,
        } => {
            let cfg = config::load_config(Some(&config))?;
            let build_secret = protect::resolve_build_secret();
            protect::protect(&protect::ProtectOptions {
                config: &cfg,
                build_dir: &build_dir,
                output_dir: &output_dir,
                build_secret: build_secret.as_deref(),
                stub_dll: stub_dll.as_deref(),
            })?;
        }
        Commands::Package {
            config,
            build_dir,
            output_dir,
            platform,
        } => {
            if platform != Platform::Macos {
                anyhow::bail!("package currently supports --platform=macos only (windows uses protect)");
            }
            let cfg = config::load_config(Some(&config))?;
            package::package_macos(&cfg, &build_dir, &output_dir)?;
        }
        Commands::Manifest {
            config,
            tag,
            artifact,
            output_dir,
            platform,
        } => {
            let cfg = config::load_config(Some(&config))?;
            manifest::generate_manifests(&cfg, &tag, &artifact, &output_dir, platform)?;
        }
        Commands::Sync {
            local_dir,
            dest,
            config,
            exclude_prefix,
        } => {
            let cfg = config::load_config(Some(&config))?;
            sync::sync(&local_dir, &dest, cfg.concurrency.upload, &exclude_prefix).await?;
        }
        Commands::UpdateChannel {
            config,
            tag,
            local,
            force,
            platform,
        } => {
            let cfg = config::load_config(Some(&config))?;
            channel::update_channel(&cfg, &tag, &local, force, platform).await?;
        }
        Commands::Verify {
            config,
            tag,
            platform,
        } => {
            let cfg = config::load_config(Some(&config))?;
            verify::verify(&cfg, &tag, platform).await?;
        }
    }
    Ok(())
}
