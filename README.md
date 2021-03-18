# NPM Registry Management

Utility for switching or enforcing registry of your `package-lock.json` file.

## Usage

Download pre-built binary from [GitHub releases](https://github.com/iFREEGROUP/nrm/releases) page.
The command name is `nrm`.

### Switching registry

```sh
nrm write --registry <registry>
```

This will update `package-lock.json` file of current directory.
You can also specify another file by appending `--path <path>` argument.

> Since v0.2, you can omit `registry` argument, and `nrm` will use npm official registry.

Example:

```sh
nrm write --registry https://registry.npmjs.org --path ./package-lock.json
```

Note: This operation will take a few minutes, depending on your network and amount of packages.

### Enforcing registry

```sh
nrm check --registry <registry>
```

This will read `package-lock.json` file of current directory and
check if it uses specific registry.
If not, this program will exit with non-zero exit code,
which can be useful in CI.

Example:

```sh
nrm check --registry https://registry.npmjs.org --path ./package-lock.json
```

> Since v0.2, you can omit `registry` argument, and `nrm` will use npm official registry.

## License

MIT License

2021-present (c) iFREE GROUP
