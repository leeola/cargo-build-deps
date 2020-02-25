extern crate clap;
extern crate serde_json;

use clap::{App, Arg};
use serde_json::Value;
use std::env;
use std::process::Command;

fn build_deps(
    target_pkg: &str,
    ignore_local_packages: bool,
    is_release: bool,
    features: Option<&str>,
    ignore_pkg: Vec<&str>,
    ignore_pkg_vers: Vec<&str>,
    with_pkgs: Vec<&str>,
) {
    let output = Command::new("cargo")
        .args(&["metadata", "--format-version", "1"])
        .output()
        .expect("Failed to execute");
    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).expect("Not UTF-8");
        panic!(stderr)
    }
    let plan = String::from_utf8(output.stdout).expect("Not UTF-8");
    let val: Value = serde_json::from_str(&plan).unwrap();
    let packages = val.get("packages").unwrap().as_array().unwrap();
    let target_cwd = packages
        .iter()
        .find_map(|node| {
            let id = node.get("id").unwrap().as_str().unwrap();
            let mut split = id.splitn(3, ' ');
            let name = split.next().unwrap();
            if name != target_pkg {
                return None;
            }
            split.next();
            let id_path_segment = split.next().expect("node id missing path");
            // TODO: is there a use-case for non-local path+file schema?
            let target_cwd = id_path_segment
                .trim_start_matches("(path+file://")
                .trim_end_matches(")");
            Some(target_cwd.to_owned())
        })
        .expect("missing target package");
    let mut pkgs: Vec<String> = packages
        .iter()
        .map(|package| {
            let id = package.get("id").unwrap().as_str().unwrap();
            let mut split = id.splitn(3, ' ');
            let name = split.next().unwrap();
            let version = split.next().unwrap();
            let source = split.next().unwrap();
            (name, version, source)
        })
        // ignore self
        .filter(|(name, _, _)| *name != target_pkg)
        .filter(|(_, _, source)| {
            if ignore_local_packages {
                !source.starts_with("(path+file://")
            } else {
                true
            }
        })
        // ignore any names that the caller chose to ignore
        .filter(|(name, _, _)| !ignore_pkg.contains(name))
        .map(|(name, version, _)| format!("{}:{}", name, version))
        // ignore any name:version's that the caller chose to ignore
        .filter(|pkg_ver| !ignore_pkg_vers.contains(&pkg_ver.as_str()))
        .collect();

    // append any user included packages
    pkgs.append(
        &mut with_pkgs
            .into_iter()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>(),
    );

    for pkg in pkgs {
        let mut command = Command::new("cargo");
        command.envs(env::vars());
        command.current_dir(&target_cwd);
        command.arg("build");
        command.arg("-p");
        command.arg(&pkg);
        if is_release {
            command.arg("--release");
        }
        if let Some(features) = features {
            command.arg("--features").arg(features);
        }

        let _exit_status = command
            .spawn()
            .expect("failed to spawn process")
            .wait()
            .expect("failed to wait process");
        // ignoring all errors as an experiment with the idea of ignoring "package not found" errors.
        // In the next commit, if this works, i'll have to string match the output i guess.
        //if !exit_status.success() {
        //    match exit_status.code() {
        //        Some(code) => panic!("Exited with status code: {}", code),
        //        None => panic!("Process terminated by signal"),
        //    }
        //}
    }
}

fn main() {
    let matched_args = App::new("cargo build-deps")
    .arg(Arg::with_name("build-deps"))
    .arg(
      Arg::with_name("package")
        .required(true)
        .takes_value(true)
    )
    .arg(Arg::with_name("dont-ignore-local-packages").long("dont-ignore-local-packages"))
    .arg(Arg::with_name("release").long("release"))
    .arg(
      Arg::with_name("features")
        .takes_value(true)
        .long("features"),
    )
    .arg(
      Arg::with_name("ignore-pkg")
        .help("ignore a specific pkg")
        .takes_value(true)
        .multiple(true)
        .long("ignore-pkg"),
    )
    .arg(
      Arg::with_name("ignore-pkg-ver")
        .help("ignore a specific pkg:ver")
        .takes_value(true)
        .multiple(true)
        .long("ignore-pkg-ver"),
    )
    .arg(
      Arg::with_name("with-pkg")
        .help("build the given dependency in addition to the others. Value is passed directly to `cargo build -p <WITH-PKG>`")
        .takes_value(true)
        .multiple(true)
        .long("with-pkg"),
    )
    .get_matches();

    let package = matched_args.value_of("package").unwrap();
    let features = matched_args.value_of("features");
    let dont_ignore_local_packages = matched_args.is_present("dont-ignore-local-packages");
    let is_release = matched_args.is_present("release");
    let ignore_pkg = matched_args
        .values_of("ignore-pkg")
        .map_or_else(|| Vec::new(), |values| values.collect::<Vec<_>>());
    let ignore_pkg_vers = matched_args
        .values_of("ignore-pkg-ver")
        .map_or_else(|| Vec::new(), |values| values.collect::<Vec<_>>());
    let with_pkgs = matched_args
        .values_of("with-pkg")
        .map_or_else(|| Vec::new(), |values| values.collect::<Vec<_>>());
    build_deps(
        package,
        !dont_ignore_local_packages,
        is_release,
        features,
        ignore_pkg,
        ignore_pkg_vers,
        with_pkgs,
    );
}
