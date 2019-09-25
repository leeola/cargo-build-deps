extern crate clap;
extern crate serde_json;

use clap::{App, Arg};
use serde_json::Value;
use std::env;
use std::process::Command;

fn build_deps(
  is_release: bool,
  features: Option<&str>,
  ignore_pkg_vers: Vec<&str>,
  with_pkgs: Vec<&str>,
) {
  let output = Command::new("cargo")
    .args(&["build", "--build-plan", "-Z", "unstable-options"])
    .output()
    .expect("Failed to execute");
  if !output.status.success() {
    let stderr = String::from_utf8(output.stderr).expect("Not UTF-8");
    panic!(stderr)
  }
  let plan = String::from_utf8(output.stdout).expect("Not UTF-8");
  let cwd = env::current_dir().unwrap();
  let val: Value = serde_json::from_str(&plan).unwrap();
  let invocations = val.get("invocations").unwrap().as_array().unwrap();
  let mut pkgs: Vec<String> = invocations
    .iter()
    .filter(|&x| {
      x.get("args").unwrap().as_array().unwrap().len() != 0
        && x.get("cwd").unwrap().as_str().unwrap() != cwd.as_os_str()
    })
    .map(|ref x| {
      let env = x.get("env").unwrap().as_object().unwrap();
      let name = env.get("CARGO_PKG_NAME").unwrap().as_str().unwrap();
      let version = env.get("CARGO_PKG_VERSION").unwrap().as_str().unwrap();
      format!("{}:{}", name, version)
    })
    .filter(|pkg_ver| !ignore_pkg_vers.contains(&pkg_ver.as_str()))
    .collect();

  // append any user included packages
  pkgs.append(
    &mut with_pkgs
      .into_iter()
      .map(|s| s.to_owned())
      .collect::<Vec<_>>(),
  );

  let mut command = Command::new("cargo");
  command.arg("build");
  for pkg in pkgs {
    command.arg("-p");
    command.arg(&pkg);
  }
  if is_release {
    command.arg("--release");
  }
  if let Some(features) = features {
    command.arg("--features").arg(features);
  }
  execute_command(&mut command);
}

fn main() {
  let matched_args = App::new("cargo build-deps")
    .arg(Arg::with_name("build-deps"))
    .arg(Arg::with_name("release").long("release"))
    .arg(
      Arg::with_name("features")
        .takes_value(true)
        .long("features"),
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

  let features = matched_args.value_of("features");
  let is_release = matched_args.is_present("release");
  let ignore_pkg_vers = matched_args
    .values_of("ignore-pkg-ver")
    .map_or_else(|| Vec::new(), |values| values.collect::<Vec<_>>());
  let with_pkgs = matched_args
    .values_of("with-pkg")
    .map_or_else(|| Vec::new(), |values| values.collect::<Vec<_>>());
  build_deps(is_release, features, ignore_pkg_vers, with_pkgs);
}

fn execute_command(command: &mut Command) {
  let mut child = command
    .envs(env::vars())
    .spawn()
    .expect("failed to execute process");

  let exit_status = child.wait().expect("failed to run command");

  if !exit_status.success() {
    match exit_status.code() {
      Some(code) => panic!("Exited with status code: {}", code),
      None => panic!("Process terminated by signal"),
    }
  }
}
