# Contributing

Pull requests and issues are welcome.

## Guidelines

- Keep `config.h` out of commits; it is gitignored.
- Do not overwrite an existing local `config.h` when preparing to build.
- Use conventional commits such as `feat:`, `fix:`, `refactor:`, and `docs:`.
- Keep one logical change per commit.

## Checks by component

The Make targets below are the same checks and builds run by CI. Run only the targets for the component you changed; there is no all-components target because the Windows companion requires Windows/MSVC while firmware and dashboard CI run on Linux. GNU Make and each component's toolchain are required.

### Firmware

Create `config.h` from `config.example.h` if it does not already exist. From the repository root:

```sh
make check-firmware
make build-firmware
```

The check runs host tests. The separate build runs PlatformIO and produces the firmware and speech-model image. CI creates `config.h` from the template.

### Dashboard

From the repository root:

```sh
make install-dashboard
make check-dashboard
make build-dashboard
```

`check-dashboard` runs formatting, lint, and tests. The build is separate.

### Windows companion

On Windows with Rust 1.92, Python 3, and the MSVC C++ build tools installed, run from the repository root:

```powershell
make check-daemon
make build-daemon
```

The check runs formatting, Clippy, tests, and localhost-fixture validation. The build compiles the Windows x64 release binary. Installer packaging remains a CI step; see the [Windows guide](docs/windows-media-daemon.md) for details. Shared core tests on other platforms do not replace Windows runtime checks.
