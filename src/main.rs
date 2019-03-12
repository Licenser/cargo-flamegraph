use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
    process::Command,
};

#[cfg(target_os = "linux")]
use inferno::collapse::perf::{Folder, Options as CollapseOptions};

#[cfg(not(target_os = "linux"))]
use inferno::collapse::dtrace::{Folder, Options as CollapseOptions};

use inferno::{
    collapse::Collapse,
    flamegraph::{from_reader, Options as FlamegraphOptions},
};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Activate release mode
    #[structopt(short = "r", long = "release")]
    release: bool,

    /// Binary to run
    #[structopt(short = "b", long = "bin")]
    bin: Option<String>,

    /// Other command to run
    #[structopt(short = "e", long = "exec")]
    exec: Option<String>,

    /// Output file, flamegraph.svg if not present
    #[structopt(parse(from_os_str), short = "o", long = "output")]
    output: Option<PathBuf>,

    /// Build features to enable
    #[structopt(short = "f", long = "features")]
    features: Option<String>,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cargo-flamegraph",
    about = "A cargo subcommand for generating flamegraphs, using inferno"
)]
enum Opts {
    #[structopt(name = "flamegraph")]
    Flamegraph(Opt),
}

#[cfg(target_os = "linux")]
mod arch {
    use super::*;

    pub const SPAWN_ERROR: &'static str = "could not spawn perf";
    pub const WAIT_ERROR: &'static str = "unable to wait for perf \
                                          child command to exit";

    pub(crate) fn initial_command(opt: &Opt) -> Command {
        let mut command = Command::new("perf");

        for arg in "record -F 99 -g".split_whitespace() {
            command.arg(arg);
        }

        let workload = workload(opt);

        for item in workload.split_whitespace() {
            command.arg(item);
        }

        command
    }

    pub fn reader() -> BufReader<File> {
        let perf_script_output = Command::new("perf")
            .arg("script")
            .output()
            .expect("unable to call perf script");
        BufReader::new(&*perf_script_output.stdout)
    }
}

#[cfg(not(target_os = "linux"))]
mod arch {
    use super::*;

    pub const SPAWN_ERROR: &'static str =
        "could not spawn dtrace";
    pub const WAIT_ERROR: &'static str =
        "unable to wait for dtrace \
         child command to exit";

    pub(crate) fn initial_command(opt: &Opt) -> Command {
        let workload = workload(opt);

        let mut command = Command::new("dtrace");

        let first = workload
            .split_whitespace()
            .nth(0)
            .expect("no command given");

        let basename: String =
            first.split('/').last().unwrap().into();

        let dtrace_script = format!(
            r#"profile-997 /execname == "{}"/ {{ @[ustack(100)] = count(); }}"#,
            basename
        );
      
        command.arg("-x");
        command.arg("ustackframes=100");

        command.arg("-n");
        command.arg(&dtrace_script);

        command.arg("-o");
        command.arg("cargo-flamegraph.stacks");

        command.arg("-c");
        command.arg(&workload);

        println!(
            "dtrace -n \"{}\" -o cargo-flamegraph.stacks -c \"{}\"",
            dtrace_script, workload
        );

        command
    }

    pub fn output() -> Vec<u8> {
        let mut buf = vec![];
        let mut f = File::open("cargo-flamegraph.stacks")
            .expect("failed to open dtrace output file cargo-flamegraph.stacks");

        use std::io::Read;
        f.read_to_end(&mut buf).expect(
            "failed to read dtrace expected \
             output file cargo-flamegraph.stacks",
        );

        std::fs::remove_file("cargo-flamegraph.stacks")
            .expect(
                "unable to remove cargo-flamegraph.stacks \
                 temporary file",
            );

        buf
    }
}

fn build(opt: &Opt) {
    if opt.exec.is_some() {
        return;
    }
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");

    if opt.release {
        cmd.arg("--release");
    }

    if let Some(ref bin) = opt.bin {
        cmd.arg("--bin");
        cmd.arg(bin);
    }

    if let Some(ref features) = opt.features {
        cmd.arg("--features");
        cmd.arg(features);
    }

    let mut child = cmd
        .spawn()
        .expect("failed to spawn cargo build command");

    let exit_status = child.wait().expect(
        "failed to wait for cargo buld child to finish",
    );

    if !exit_status.success() {
        eprintln!("cargo build failed: {:?}", child.stderr);
        std::process::exit(1);
    }
}

fn workload(opt: &Opt) -> String {
    if let Some(ref exec) = opt.exec {
        return exec.clone();
    }

    let mut metadata_cmd =
        cargo_metadata::MetadataCommand::new();
    metadata_cmd.no_deps();
    let metadata = metadata_cmd
        .exec()
        .expect("could not access crate metadata");

    let mut binary_path = metadata.target_directory;

    if opt.release {
        binary_path.push("release");
    } else {
        binary_path.push("debug");
    }

    let targets: Vec<String> = metadata
        .packages
        .into_iter()
        .flat_map(|p| p.targets)
        .filter(|t| t.crate_types.contains(&"bin".into()))
        .map(|t| t.name)
        .collect();

    if targets.is_empty() {
        eprintln!(
            "no binary targets found, maybe you \
             wanted to pass the --exec argument \
             to cargo flamegraph?"
        );
        std::process::exit(1);
    }

    let target: &String = if let Some(ref bin) = opt.bin {
        if targets.contains(&bin) {
            bin
        } else {
            eprintln!(
                "could not find desired target {} \
                 in the targets for this crate: {:?}",
                bin, targets
            );
            std::process::exit(1);
        }
    } else if targets.len() == 1 {
        &targets[0]
    } else {
        eprintln!(
            "several possible targets found: {:?}, \
             please pass the --bin argument to cargo flamegraph \
             to choose one of them",
            targets
        );
        std::process::exit(1);
    };

    binary_path.push(target);

    binary_path.to_string_lossy().into()
}

fn main() {
    let Opts::Flamegraph(mut opt) = Opts::from_args();

    build(&opt);

    let flamegraph_filename = opt
        .output
        .take()
        .unwrap_or("flamegraph.svg".into());

    let mut command = arch::initial_command(&opt);

    let mut recorder =
        command.spawn().expect(arch::SPAWN_ERROR);
    let exit_status =
        recorder.wait().expect(arch::WAIT_ERROR);

    if !exit_status.success() {
        eprintln!("failed to sample program");
        std::process::exit(1);
    }

    let reader = arch::reader();

    let mut collapsed = vec![];

    let collapsed_writer = BufWriter::new(&mut collapsed);

    let collapse_options = CollapseOptions::default();

    Folder::from(collapse_options)
        .collapse(reader, collapsed_writer)
        .expect("unable to collapse generated profile data");

    let collapsed_reader = BufReader::new(&*collapsed);

    let flamegraph_file =
        File::create(flamegraph_filename).expect("unable to create flamegraph.svg output file");

    let flamegraph_writer = BufWriter::new(flamegraph_file);

    let flamegraph_options = FlamegraphOptions::default();

    from_reader(flamegraph_options, collapsed_reader, flamegraph_writer)
        .expect("unable to generate a flamegraph from the collapsed stack data");
}
