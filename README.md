<p align="center">
  <h1 align="center">cf-pages-cli</h1>
</p>

**Command line utility for managing Cloudflare Pages projects**

[![crates-badge](https://img.shields.io/crates/v/cf-pages-cli.svg)](https://crates.io/crates/cf-pages-cli)

## What is `cf-pages-cli`

`cf-pages-cli` is a line utility for managing [Cloudflare Pages](https://pages.cloudflare.com/) projects. Currently, its only capability is managing environment variables, which is useful in CI/CD workflows to synchronize projects with variables stored in Git.

## Installation

With `cargo` installed, install from crates.io:

```console
cargo install --locked --version 0.2.0 cf-pages-cli
```

## Usage

### Synchronize variables

First, make sure you have your Cloudflare account ID, as well as a valid Cloudflare API token (with the `Cloudflare Pages:Edit` permission). Export them as environment variables:

```console
$ export CLOUDFLARE_ACCOUNT="YOUR_ACCOUNT_ID"
$ export CLOUDFLARE_TOKEN="YOUR_API_TOKEN"
```

_(It's also possible to use them as command line options via `--account` and `--token`, respectively, but it's easier to just export them as they're used in many commands.)_

By default, the command exports the latest settings for both the production and preview environments. You can also export the variables from a specific deployment by adding a `--deployment DEPLOYMENT_ID` option. Note that since each deployment only targets one environment, the other environment will be left as `null` in the resulting JSON file.

Then, export the environment variables of your project:

```console
$ cf-pages get-env-vars --project YOUR_PROJECT_NAME --output ./vars.json
Environment variables written to: ./vars.json
```

_(It's also possible to set the project name and file path via the `CF_PAGES_PROJECT` and `CF_PAGES_OUTPUT` environment variables, respectively.)_

You can also print the generated file content to stdout by omitting the `--output` option.

Now, make changes to the `vars.json` file, and upload to Cloudflare:

```console
$ cf-pages set-env-vars --project YOUR_PROJECT_NAME --file ./vars.json
Environment variables successfully updated
```

### Generate `.env` files

The `vars.json` file exported with the `get-env-vars` can also be used to generate `.env` files, which are useful for front-end development:

```console
$ cf-pages to-env-file --output ./.env ./vars.json
Environment variables written to: ./.env
```

You can also print the generated file content to stdout by omitting the `--output` option.

By default, environment variables for the production environment are exported. To export the preview environment instead, add the `--environment preview` option.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
