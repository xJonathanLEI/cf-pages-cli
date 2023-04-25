<p align="center">
  <h1 align="center">cf-pages-cli</h1>
</p>

**Command line utility for managing Cloudflare Pages projects**

## What is `cf-pages-cli`

`cf-pages-cli` is a line utility for managing [Cloudflare Pages](https://pages.cloudflare.com/) projects. Currently, its only capability is managing environment variables, which is useful in CI/CD workflows to synchronize projects with variables stored in Git.

## Installation

With `cargo` installed, install from crates.io:

```console
cargo install --locked --version 0.1.0 cf-pages-cli
```

## Usage

First, make sure you have your Cloudflare account ID, as well as a valid Cloudflare API token (with the `Cloudflare Pages:Edit` permission). Export them as environment variables:

```console
$ export CLOUDFLARE_ACCOUNT="YOUR_ACCOUNT_ID"
$ export CLOUDFLARE_TOKEN="YOUR_API_TOKEN"
```

_(It's also possible to use them as command line options via `--account` and `--token`, respectively, but it's easier to just export them as they're used in all commands.)_

Then, export the environment variables of your project:

```console
$ cf-pages get-env-vars --project YOUR_PROJECT_NAME --path ./vars.json
Environment variables written to: ./vars.json
```

_(It's also possible to set the project name and file path via the `CF_PAGES_PROJECT` and `CF_PAGES_PATH` environment variables, respectively.)_

Now, make changes to the `vars.json` file, and upload to Cloudflare:

```console
$ cf-pages set-env-vars --project YOUR_PROJECT_NAME --path ./vars.json
Environment variables successfully updated
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
