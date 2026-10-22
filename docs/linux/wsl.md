# Build and test in WSL

How to build the project and run its tests in Linux under the Windows Subsystem for Linux (WSL 2). This
is the Linux setup the project is verified on: Ubuntu 24.04 on WSL 2, on the Windows 11 reference
machine ([Linux support PRD](../prd/chicago-crime/soporte-linux.prd.md), decision L1). On a machine
that runs Linux directly, follow the [native Linux guide](native.md) instead.

## 1. Get a Linux distribution

From PowerShell, check what is installed:

```powershell
wsl -l -v
```

The distribution must show `VERSION 2`. If there is none, install Ubuntu 24.04 and create its user when
it asks:

```powershell
wsl --install -d Ubuntu-24.04
```

Every command below runs inside that distribution. Open it with `wsl -d Ubuntu-24.04`, or with the
name `wsl -l -v` prints (for example `Ubuntu`).

## 2. Install the toolchain

Rust links Linux programs with the system C toolchain, so install it together with Git and curl, then
install Rust with rustup:

```bash
sudo apt update
sudo apt install -y build-essential git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. ~/.cargo/env
cargo --version
```

The project needs at least Rust 1.85 (edition 2024). It was verified with Rust 1.99.

## 3. Put the code in the Linux file system

Clone the repository into your Linux home folder, not into a Windows drive under `/mnt/c`:

```bash
git clone https://github.com/jesuslpm/chicago-crime.git ~/chicago-crime
cd ~/chicago-crime
```

If you already have a Windows checkout with local changes, you can clone that one instead:
`git clone /mnt/c/path/to/chicago-crime ~/chicago-crime`.

Why not build in `/mnt/c` directly: every file access there crosses from Linux to Windows, which makes a
build that reads and writes thousands of small files much slower, and a `target` folder shared with the
Windows build would mix the binaries of both systems. The project is only verified on the Linux file
system.

## 4. Copy the reference CSV (only to run the applications)

The tests do not need the CSV: they build their own small files. The CSV is only needed to run the
generator and the query app on real data. It is not in the repository (Git ignores it), so copy it
from Windows, or download it again as the [README](../../README.md#get-the-dataset) explains:

```bash
cp /mnt/c/path/to/chicago-crime-data.csv ~/chicago-crime/
```

It is about 2.4 GB. A new download has more rows than the reference extract, so the exact figures in the
PRDs only hold for the copy of that extract.

## 5. Build and test

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Every test passes. One test needs a folder that cannot be deleted; as `root` nothing stops a deletion,
so it prints `skipped: this user can delete any folder` and passes without checking anything. Run the
tests as a normal user.

## 6. Run the applications

The binaries are in `target/release/`, without the `.exe` extension:

```bash
./target/release/crime-generator --input chicago-crime-data.csv --output ~/datasets/chicago
./target/release/crime-query --dataset ~/datasets/chicago --where "Primary Type=THEFT" --where District=008 --count
./target/release/crime-canopy-generator --input chicago-crime-data.csv --output ~/datasets/chicago-canopy
./target/release/crime-canopy-query --dataset ~/datasets/chicago-canopy --where "Primary Type=THEFT" --where District=008 --count
```

The acceptance scripts of the CanopyDB generator and of the CanopyDB query app run only on Windows. On
Linux the generator's memory criterion (AC-M4) is checked by hand: run the generator on the full reference
CSV and read the line `Pico de memoria privada comprometida` of its summary, which must stay within
512 MiB. The query app's memory criterion (AC-M8) is checked on Windows only, and the comparison bench of
the two pairs (`scripts/acceptance/run-canopy-comparison.ps1`) runs only on Windows too.

The robustness scenarios run on Linux too, with the same binary and options as on Windows, from the copy in
the Linux file system (see
[Robustness scenarios](../../README.md#robustness-scenarios)). R13, the big CSV, comes out as not
applicable:

```bash
cargo build --release --workspace
./target/release/crime-canopy-scenarios --csv chicago-crime-data.csv --work work/scenarios
```

Add the rows it writes to `work/scenarios/registro-Linux-<fecha>.md` to the scenario log,
[`docs/canopy-db/escenarios.md`](../canopy-db/escenarios.md).

Every option and message is the same as on Windows (see the [README](../../README.md)). Keep the
datasets in the Linux file system too, for the same reason as the code. Windows can still open them
through `\\wsl$\Ubuntu-24.04\home\<user>\datasets`.

## What differs from Windows

[Linux notes](native.md#what-differs-from-windows): the memory figure and what still only runs on
Windows are the same in WSL and on native Linux.
