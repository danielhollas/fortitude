# Configuration

Fortitude will look for either a `fortitude.toml` or `fpm.toml` file
in the current directory, or one of its parents. If using
`fortitude.toml`, settings should be under the command name, while for
`fpm.toml` files, this has to be additionally nested under the
`extra.fortitude` table:


=== "fortitude.toml"

    ```toml
    [check]
    select = ["S", "T"]
    ignore = ["S001", "S051"]
    line-length = 132
    ```
=== "fpm.toml"

    ```toml
    [extra.fortitude.check]
    select = ["S", "T"]
    ignore = ["S001", "S051"]
    line-length = 132
    ```

You can use `--extend-select` from the command line to select additional
rules on top of those in the configuration file.

```bash
# Selects S, T, and M categories
fortitude check --extend-select=M
```

## Full command-line interface

See `fortitude help` for the full list of Fortitude's top-level commands:

<!-- Begin auto-generated command help. -->

```text
A Fortran linter, written in Rust and installable with Python

Usage: fortitude [OPTIONS] <COMMAND>

Commands:
  check    Perform static analysis on files and report issues
  explain  Get descriptions, rationales, and solutions for each rule
  help     Print this message or the help of the given subcommand(s)

Options:
      --config-file <CONFIG_FILE>  Path to a TOML configuration file
  -h, --help                       Print help
  -V, --version                    Print version
```

<!-- End auto-generated command help. -->

Or `fortitude help check` for more on the linting command:

<!-- Begin auto-generated check help. -->

```text
Perform static analysis on files and report issues

Usage: fortitude check [OPTIONS] [FILES]...

Arguments:
  [FILES]...  List of files or directories to check. Directories are searched recursively for Fortran files. The `--file-extensions` option can be used to control which files are included in the search [default: .]

Options:
      --line-length <LINE_LENGTH>
          Set the maximum allowable line length [default: 100]
      --file-extensions <FILE_EXTENSIONS>
          File extensions to check [default: f90 F90 f95 F95 f03 F03 f08 F08 f18 F18 f23 F23]
      --fix
          Apply fixes to resolve lint violations. Use `--no-fix` to disable or `--unsafe-fixes` to include unsafe fixes
      --unsafe-fixes
          Include fixes that may not retain the original intent of the code. Use `--no-unsafe-fixes` to disable
      --show-fixes
          Show an enumeration of all fixed lint violations. Use `--no-show-fixes` to disable
      --fix-only
          Apply fixes to resolve lint violations, but don't report on, or exit non-zero for, leftover violations. Implies `--fix`. Use `--no-fix-only` to disable or `--unsafe-fixes` to include unsafe fixes
      --output-format <OUTPUT_FORMAT>
          Output serialization format for violations. The default serialization format is "full" [env: FORTITUDE_OUTPUT_FORMAT=] [possible values: concise, full, json, json-lines, junit, grouped, github, gitlab, pylint, rdjson, azure, sarif]
      --preview
          Enable preview mode; checks will include unstable rules and fixes. Use `--no-preview` to disable
      --progress-bar <PROGRESS_BAR>
          Progress bar settings. Options are "off" (default), "ascii", and "fancy" [possible values: off, fancy, ascii]
  -h, --help
          Print help

Rule selection:
      --ignore <RULE_CODE>
          Comma-separated list of rules to ignore
      --select <RULE_CODE>
          Comma-separated list of rule codes to enable (or ALL, to enable all rules)
      --extend-select <RULE_CODE>
          Like --select, but adds additional rule codes on top of those already specified
```

<!-- End auto-generated check help. -->
