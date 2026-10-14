# Build and test on Linux

How to build the project and run its tests on a machine that runs Linux directly. Under Windows, follow
the [WSL guide](wsl.md) instead: it is the same process plus getting a distribution and keeping the
files out of the Windows drives.

The project is verified on Ubuntu 24.04 under WSL 2, which runs a real Linux kernel ([Linux support
PRD](../prd/chicago-crime/soporte-linux.prd.md), decision L1). It is not verified on another machine,
but nothing in it depends on WSL.

## 1. Install the toolchain

Rust links Linux programs with the system C toolchain. Install it with Git and curl, using your
distribution's package manager:

| Distribution | Command |
|---|---|
| Debian, Ubuntu | `sudo apt install -y build-essential git curl` |
| Fedora | `sudo dnf install -y gcc git curl` |
| Arch | `sudo pacman -S --needed base-devel git curl` |

Then install Rust with rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. ~/.cargo/env
cargo --version
```

The project needs at least Rust 1.85 (edition 2024). It was verified with Rust 1.99.

## 2. Get the code and, to run the applications, the CSV

```bash
git clone https://github.com/jesuslpm/chicago-crime.git ~/chicago-crime
cd ~/chicago-crime
```

The tests do not need the CSV. To run the applications on real data, put `chicago-crime-data.csv` at
the repository root: download it as the [README](../../README.md#get-the-dataset) explains, using
`curl -L` instead of `curl.exe -L`, or copy the reference extract from another machine. Git ignores
it.

## 3. Build and test

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Every test passes. A few tests need a folder that cannot be deleted; as `root` nothing stops a deletion,
so they print `skipped: this user can delete any folder` and pass without checking anything. Run the
tests as a normal user.

## 4. Run the applications

The binaries are in `target/release/`, without the `.exe` extension:

```bash
./target/release/crime-generator --input chicago-crime-data.csv --output ~/datasets/chicago
./target/release/crime-query --dataset ~/datasets/chicago --where "Primary Type=THEFT" --where District=008 --count
```

Every option and message is the same as on Windows (see the [README](../../README.md)).

## What differs from Windows

- **Memory figure.** The generator's summary line `Pico de memoria privada comprometida` and its
  over-the-limit warning show, on Linux, the peak resident memory of the process (`VmHWM`), which the
  kernel keeps from the start of the process, as Windows keeps its peak of committed memory ([Linux
  support PRD](../prd/chicago-crime/soporte-linux.prd.md), decision L3). The two are close but not the
  same: the Linux figure also counts the program's own code, and does not count memory the system moved
  to swap.
- **Leftovers of interrupted runs.** As on Windows, a run discards the sort folders of a generator that
  is no longer running. On Linux a running sort also holds an advisory lock on its folder, and a held
  folder is never deleted, so the folders of a generator in another container that shares the
  temporary folder are safe too. That needs a file system with advisory locks, which local file systems
  have; on a network file system without them, give each container its own temporary folder.
- **Acceptance scripts.** The scripts in `scripts/acceptance` are PowerShell scripts that use Windows
  APIs, and run only on Windows. On Linux, the independent verifier still runs on its own:
  `./target/release/crime-acceptance --csv chicago-crime-data.csv --dataset ~/datasets/chicago`.
